use std::{convert::Infallible, str::FromStr};

use pulldown_cmark::{
    CowStr, Event, Tag
};


use crate::{
    ir,
    compiler::URL,
};

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

    pub fn link(&self) -> Result<(URL, &str), LinkExtractionError> {
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

        Ok((uri, &title))
    }

    pub fn uri(&self) -> Result<URL, LinkExtractionError> {
        Ok(self.link()?.0)
    }

    pub fn text(&self) -> impl Iterator<Item = &str> {
        self.0.iter().filter_map(|event| {
            if let Event::Text(text) = event {
                Some(text.as_ref())
            } else {
                None
            }
        })
    }

    pub fn str(&self) -> CowStr {
        let mut iter = self.text();
        let Some(s) = iter.next() else {
            return CowStr::Borrowed("")
        };

        if let Some(s2) = iter.next() {
            let string = iter.fold(s.to_string() + s2, |acc, s| acc + s);
            return CowStr::Boxed(string.into_boxed_str())
        }

        CowStr::Borrowed(s)
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
            vec![Event::Text(CowStr::from(s).into_static())]
        ))
    }
}

#[derive(Clone, Debug)]
enum RichTextStorage {
    CommonMarkEvents(CommonMark)
}

impl RichTextStorage {
    fn str(text: &str, representation: RichTextRepresentation) -> Self {
        match representation {
            RichTextRepresentation::CommonMark =>
                Self::CommonMarkEvents(CommonMark::from_str(text).unwrap()),
        }
    }

    fn empty(representation: RichTextRepresentation) -> Self {
        match representation {
            RichTextRepresentation::CommonMark =>
                Self::CommonMarkEvents(CommonMark::default()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum RichTextRepresentation {
    CommonMark,
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
            storage: RichTextStorage::CommonMarkEvents(events)
        }
    }

    pub fn link(&self) -> Result<(url::Url, &str), LinkExtractionError> {
        match &self.storage {
            RichTextStorage::CommonMarkEvents(events) => 
                events.link()
        }
    }

    pub fn str(&self) -> CowStr {
        match &self.storage {
            RichTextStorage::CommonMarkEvents(common_mark) => return common_mark.str(),
        }
    }

    pub fn append(&mut self, content: RichText) {
        
    }
}

impl ToString for RichText {
    fn to_string(&self) -> String {
        match &self.storage {
            RichTextStorage::CommonMarkEvents(events) => events.to_string(),
        }
    }
}