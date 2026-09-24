use std::{borrow::Cow, convert::Infallible, str::FromStr};
use fallible_iterator::{convert, FallibleIterator};

use pulldown_cmark::{
    BlockQuoteKind, Event, Tag
};
use serde::{Deserialize, Serialize};
use strum_macros::EnumString;

use crate::{
    compiler::{URL, strings::CowStr}, ir,
};

#[derive(Clone, Debug, Eq, PartialEq, EnumString)]
pub enum ListStyle {
    #[strum(serialize = "ordered")]
    Ordered,

    #[strum(serialize = "unordered")]
    Unordered,
}

#[derive(Clone, Debug, Eq, PartialEq, EnumString)]
pub enum InlineTextStyle {
    #[strum(serialize = "emphasized")]
    Emphasized,

    #[strum(serialize = "bold")]
    Bold,

    #[strum(serialize = "strikethrough")]
    Strikethrough,

    #[strum(serialize = "underlined")]
    Underlined,
}

// In the IR, we don't need to know what the original 'base format' was.
// For instance, Doxygen, DocC, RustDoc, use Markdown as the base format.
// Here, we only care about how it is stored. If it is stored as
// pulldown-cmark events, then we only have to know how to handle those.
// As consequence, there may be multiple storage types for the same original
// format just due to different parser being used.
// If we're going to store something like Markdown as a raw text buffer in IR,
// then that would have its own storage type in RichTextStorage.
// The same logic also applies to the DocVariant. In IR, we don't care about details
// needed while parsing richt text containing documentation instructions.
// Now, we only care about what we expect when we handle what's in RichTextStorage.
// This might be necessary to handle behavior such as determining how links are to
// be resolved.

#[derive(thiserror::Error, Debug, Clone)]
pub enum LinkExtractionError {
    #[error("Missing link")]
    MissingLink,

    #[error("More than one link")]
    TooManyLinks,

    #[error("Malformed URI, {0}")]
    MalformedURI(#[from] url::ParseError),
}

#[derive(Clone, Debug)]
pub struct CommonMark(Vec<pulldown_cmark::Event<'static>>);

impl CommonMark {
    pub fn new(events: Vec<pulldown_cmark::Event<'static>>) -> Self {
        Self(events)
    }

    fn from_cow_str(s: CowStr) -> Self {
        let cow: pulldown_cmark::CowStr = s.into();
        Self::new(
            vec![Event::Text(cow.into_static())]
        )
    }

    pub fn link(&self) -> Result<(URL, CowStr<'static>), LinkExtractionError> {
        let mut links = self.0.iter().filter_map(|event| {
            if let Event::Start(Tag::Link { 
                dest_url, 
                title, 
                .. 
            }) = event {
                Some((dest_url, title))
            } else {
                None
            }
        });

        let (url, title) = links.next()
            .ok_or(LinkExtractionError::MissingLink)?;

        if links.next().is_some() {
            return Err(LinkExtractionError::TooManyLinks)
        }

        let uri = URL::parse(&url)?;

        Ok((uri, title.clone().into()))
    }

    pub fn uri(&self) -> Result<URL, LinkExtractionError> {
        Ok(self.link()?.0)
    }

    pub fn _text(&self) -> impl Iterator<Item = &pulldown_cmark::CowStr<'static>> {
        self.0.iter().filter_map(|event| {
            if let Event::Text(text) = event {
                Some(text)
            } else {
                None
            }
        })
    }

    pub fn str(&self) -> CowStr<'static> {
        let mut iter = self._text();
        let Some(s) = iter.next() else {
            return CowStr::Borrowed("")
        };

        if let Some(s2) = iter.next() {
            let string = iter.fold(s.string() + s2, |acc, s| acc + s);
            return CowStr::Owned(string)
        }

        s.clone().into()
    }

    pub fn try_map_text<E>(&self, f: impl Fn(&str) -> Result<CowStr<'static>, E>) -> Result<Self, E> {
        let events = convert(self.0.iter().map(|e| 
            Ok(match e {
                pulldown_cmark::Event::Text(s) => pulldown_cmark::Event::Text(f(&s)?.into()),
                pulldown_cmark::Event::Code(s) => pulldown_cmark::Event::Code(f(&s)?.into()),
                pulldown_cmark::Event::Html(s) => pulldown_cmark::Event::Html(f(&s)?.into()),
                pulldown_cmark::Event::InlineHtml(s) => pulldown_cmark::Event::InlineHtml(f(&s)?.into()),
                pulldown_cmark::Event::InlineMath(s) => pulldown_cmark::Event::InlineMath(f(&s)?.into()),
                pulldown_cmark::Event::DisplayMath(s) => pulldown_cmark::Event::DisplayMath(f(&s)?.into()),
                pulldown_cmark::Event::Start(tag) => 
                    pulldown_cmark::Event::Start(match tag {
                        pulldown_cmark::Tag::Heading { 
                            level, 
                            id, 
                            classes, 
                            attrs 
                        } => pulldown_cmark::Tag::Heading { 
                                level: level.clone(), 
                                id: id.as_ref().map(|id| f(&id)).transpose()?.map(Into::into), 
                                classes: convert(classes.iter().map(|c| Ok(f(&c)?.into()))).collect()?, 
                                attrs: convert(attrs.iter().map(|(k,v)| Ok((f(&k)?.into(), v.as_ref().map(|v| f(&v)).transpose()?.map(Into::into))))).collect()?, 
                            },
                        pulldown_cmark::Tag::Link { 
                            link_type, 
                            dest_url, 
                            title, 
                            id 
                        } => pulldown_cmark::Tag::Link { 
                            link_type: link_type.clone(),
                            dest_url: f(&dest_url)?.into(),
                            title: f(&title)?.into(),
                            id: f(&id)?.into(),
                        },
                        pulldown_cmark::Tag::Image { 
                            link_type, 
                            dest_url, 
                            title, 
                            id 
                        } => pulldown_cmark::Tag::Image { 
                            link_type: link_type.clone(),
                            dest_url: f(&dest_url)?.into(),
                            title: f(&title)?.into(),
                            id: f(&id)?.into(),
                        },
                        pulldown_cmark::Tag::DocTag { 
                            name,
                            arguments,
                            argument_delimiters
                        } => pulldown_cmark::Tag::DocTag { 
                            name: name.clone(),
                            arguments: convert(arguments.iter()
                                .map(|(name,typ,value)| 
                                    Ok((name.clone(), typ.clone(), f(&value)?.into()))
                                 )).collect()?,
                            argument_delimiters: argument_delimiters.clone(),
                        },
                        t => t.clone()
                    }),
                e => e.clone()
            })
        )).collect()?;
        Ok(Self(events))
    }
}

impl Default for CommonMark {
    fn default() -> Self {
        Self(Vec::default())
    }
}

impl ToString for CommonMark {
    fn to_string(&self) -> String {
        self.0
            .iter()
            .filter_map(|e| match e {
                Event::Text(s) => Some(s.as_ref()),
                _ => None
            })
            .fold(String::new(), |acc, s| acc + s)
    }
}

impl Into<RichText> for CommonMark {
    fn into(self) -> RichText {
        RichText::common_mark(self)
    }
}

impl FromStr for CommonMark {
    type Err = Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(
            vec![Event::Text(pulldown_cmark::CowStr::from(s).into_static())]
        ))
    }
}

#[derive(Clone, Debug)]
enum RichTextStorage {
    CommonMark(CommonMark)
}

impl RichTextStorage {
    fn str(text: &str, representation: RichTextRepresentation) -> Self {
        match representation {
            RichTextRepresentation::CommonMark =>
                Self::CommonMark(CommonMark::from_str(text).unwrap()),
        }
    }

    fn empty(representation: RichTextRepresentation) -> Self {
        match representation {
            RichTextRepresentation::CommonMark =>
                Self::CommonMark(CommonMark::default()),
        }
    }

    fn into_admonition(self, kind: AdmonitionKind) -> Self {
        match self {
            Self::CommonMark(events) => {
                let kind = Some(kind.into());
                let mut events = events;
                events.0.insert(
                    0,
                    Event::Start(Tag::BlockQuote(kind))
                );
                events.0.push(Event::End(pulldown_cmark::TagEnd::BlockQuote(kind)));
                
                Self::CommonMark(events)
            }
        }
    }

    pub fn try_map_text<E>(&self, f: impl Fn(&str) -> Result<CowStr<'static>, E>) -> Result<Self, E> {
        Ok(match self {
            Self::CommonMark(md) => Self::CommonMark(md.try_map_text(f)?)
        })
    }
}

#[derive(Clone, Debug)]
pub enum RichTextRepresentation {
    CommonMark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, EnumString, Serialize, Deserialize)]
pub enum AdmonitionKind {
    #[strum(serialize = "note")]
    Note,

    #[strum(serialize = "tip")]
    Tip,

    #[strum(serialize = "important")]
    Important,

    #[strum(serialize = "warning")]
    Warning,

    #[strum(serialize = "caution")]
    Caution,
}

impl From<AdmonitionKind> for BlockQuoteKind {
    fn from(value: AdmonitionKind) -> Self {
        match value {
            AdmonitionKind::Caution => Self::Caution,
            AdmonitionKind::Important => Self::Important,
            AdmonitionKind::Warning => Self::Warning,
            AdmonitionKind::Note => Self::Note,
            AdmonitionKind::Tip => Self::Tip,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RichText {
    dialect: Option<ir::Dialect>,

    storage: RichTextStorage,
}

impl Default for RichText {
    fn default() -> Self {
        Self::empty(RichTextRepresentation::CommonMark)
    }
}

impl RichText {
    pub fn from_str(text: &str, representation: RichTextRepresentation) -> Self {
        Self {
            dialect: None,
            storage: RichTextStorage::str(text, representation)
        }
    }

    pub fn empty(representation: RichTextRepresentation) -> Self {
        Self {
            dialect: None,
            storage: RichTextStorage::empty(representation)
        }
    }

    pub fn common_mark(events: CommonMark) -> Self {
        Self {
            dialect: None,
            storage: RichTextStorage::CommonMark(events)
        }
    }

    pub fn link(&self) -> Result<(url::Url, CowStr<'static>), LinkExtractionError> {
        match &self.storage {
            RichTextStorage::CommonMark(events) => 
                events.link()
        }
    }

    pub fn str(&self) -> CowStr {
        match &self.storage {
            RichTextStorage::CommonMark(common_mark) => return common_mark.str(),
        }
    }

    pub fn append(&mut self, content: RichText) {
        
    }

    pub fn into_admonition(self, kind: AdmonitionKind) -> Self {
        Self { 
            dialect: self.dialect, 
            storage: self.storage.into_admonition(kind) 
        }
    }

    pub fn try_map_text<E>(&self, f: impl Fn(&str) -> Result<CowStr<'static>, E>) -> Result<Self, E> {
        Ok(Self {
            storage: self.storage.try_map_text(f)?,
            dialect: self.dialect
        })
    }

    pub fn map_text(&self, f: impl Fn(&str) -> CowStr<'static>) -> Self {
        self.try_map_text::<Infallible>(|s| Ok(f(s))).unwrap()
    }
}

impl ToString for RichText {
    fn to_string(&self) -> String {
        match &self.storage {
            RichTextStorage::CommonMark(events) => events.to_string(),
        }
    }
}