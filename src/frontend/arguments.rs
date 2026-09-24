use std::{any::Any, borrow::Cow, convert::Infallible, matches, num::ParseIntError, ops::Index, path::PathBuf, str::{FromStr, ParseBoolError}};
use strum_macros::AsRefStr;
use crate::{backend, compiler::{URL, diagnostics::{self, EditorLocation, MinimalLocation}, strings::CowStr}, ir::{self, Language, RichText, ValueType, rich_text::{LinkExtractionError, RichTextRepresentation}}};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateComponent<'a> {
    /// A named slot that is supposed to be replaced by a literal
    Slot(CowStr<'a>),

    /// A literal that is supposed to appear verbatim in the result
    Literal(CowStr<'a>),
}

pub type TemplateSlice<'a> = [TemplateComponent<'a>];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Template<'a> {
    pub capacity: usize,
    pub components: Cow<'a, TemplateSlice<'a>>
}

impl<'a> Template<'a> {
    pub const SIGN: char = '$';
    pub const fn _analyze(input: &str, sign: char) -> (usize, usize) {
        let bytes = input.as_bytes();
        let mut count = 0;
        let mut i: usize = 0;
        let mut text_start = 0;
        let mut min_capacity = 0;

        const fn closing_delimiter(opening: char) -> char {
            match opening {
                '{' => '}',
                '(' => ')',
                _ => std::panic!("lolz check before")
            }
        }

        while i < bytes.len() {
            if bytes[i] == sign as u8 {
                if i > text_start {
                    count += 1;
                    min_capacity += i - text_start;
                }
                i += 1;
                

                if i < bytes.len() && matches!(bytes[i] as char, '{' | '(') {
                    let closing = closing_delimiter(bytes[i] as char);
                    i += 1;
                    while i < bytes.len() && bytes[i] as char != closing {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1; 
                    }
                } else {
                    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                        i += 1;
                    }
                }
                
                count += 1;
                text_start = i;
            } else {
                i += 1;
            }
        }

        if text_start < bytes.len() {
            count += 1;
            min_capacity += bytes.len() - text_start;
        }

        (count, min_capacity)
    }

    pub const fn _parse_template_const<'b: 'a, const N: usize>(input: &'b str, sign: char) -> [TemplateComponent<'b>; N] {
        let mut parts = [const { TemplateComponent::Literal(CowStr::Borrowed("")) }; N];
        Self::_parse_template(input, sign, &mut parts);
        parts
    }

    pub const fn _parse_template<'b: 'a>(input: &'b str, sign: char, parts: &mut [TemplateComponent<'b>]) {
        let bytes = input.as_bytes();
        let mut part_idx = 0;
        let mut i = 0;
        let mut text_start = 0;
        

        while i < bytes.len() {
            if bytes[i] == sign as u8 {
                if i > text_start {
                    unsafe {
                        let ptr = bytes.as_ptr().add(text_start);
                        let text = std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, i - text_start));
                        std::ptr::write(&mut parts[part_idx], TemplateComponent::Literal(CowStr::Borrowed(text)));
                    }
                    part_idx += 1;
                }
                i += 1;
                let var_start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                unsafe {
                    let ptr = bytes.as_ptr().add(var_start);
                    let slot = std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, i - var_start));
                    std::ptr::write(&mut parts[part_idx], TemplateComponent::Slot(CowStr::Borrowed(slot)));
                }
                part_idx += 1;
                text_start = i;
            } else {
                i += 1;
            }
        }

        if text_start < bytes.len() {
            unsafe {
                let ptr = bytes.as_ptr().add(text_start);
                let text = std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, bytes.len() - text_start));
                std::ptr::write(&mut parts[part_idx], TemplateComponent::Literal(CowStr::Borrowed(text)));
            }
        }
    }

    pub const fn _from_components(components: &'a [TemplateComponent<'a>], min_capacity: usize) -> Self {
        Self {
            components: Cow::Borrowed(components),
            capacity: min_capacity
        }
    }
}

#[macro_export]
macro_rules! template {
    ($s:expr) => {{
        const A: (usize, usize) = Template::_analyze($s, Template::SIGN);
        const N: usize = A.0;
        static PARTS: [TemplateComponent<'static>; N] = Template::_parse_template_const::<N>($s, Template::SIGN);
        Template::_from_components(&PARTS, A.1)
    }};
}

pub use template;

#[derive(Clone, Debug)]
pub enum Parameter<T, F: for<'a> Fn(&Arguments<'a>) -> T = for<'a> fn(&Arguments<'a>) -> T> {
    Variable(CowStr<'static>), // parameter name
    // Templated(T, Vec<String>), // template, parameter names
    Constant(T),

    Template(Template<'static>),

    Func(F),

    #[deprecated(note = "Defaults are the first step towards expressions, figure out how to do that first.")]
    VariableDefault(CowStr<'static>, T),
}

impl<T> From<T> for Parameter<Option<T>> {
    fn from(value: T) -> Self {
        Self::Constant(Some(value))
    }
}

#[macro_export]
macro_rules! param {
    ($name:expr, $default:expr) => {
        Parameter::VariableDefault($name.into(), $default)
    };

    ($name:expr) => {
        Parameter::Variable($name.into())
    };
}

pub(crate) use param;

#[macro_export]
macro_rules! func {
    ($func:expr) => {
        Parameter::Func($func)
    };
}

pub(crate) use func;

#[macro_export]
macro_rules! v {
    ($value:expr) => {
        Parameter::Constant($value.into())
    };
}

pub(crate) use v;

#[macro_export]
macro_rules! s {
    ($value:expr) => {
        Parameter::Constant(crate::compiler::strings::CowStr::from($value).into())
    };
}

pub(crate) use s;


#[macro_export]
macro_rules! t {
    ($value:expr) => {
        Parameter::Template(template!($value))
    };
}

pub(crate) use t;

#[derive(Clone, Debug)]
pub struct Argument<'a> {
    pub name: &'a str,
    pub value: ArgumentValue<'a>,
    pub location: MinimalLocation,
}

#[derive(Debug, Clone)]
pub enum ArgumentValue<'a> {
    Void,

    PlainText(CowStr<'a>),

    RichText(RichText),
}

impl<'a> ArgumentValue<'a> {
    pub fn from_opt_str(s: Option<&'a str>) -> Self {
        s.map_or(Self::Void, |s| Self::PlainText(s.into()))
    }

    pub fn str(&'a self) -> CowStr<'a> {
        match self {
            ArgumentValue::Void => CowStr::Borrowed(""),
            ArgumentValue::PlainText(s) => s.clone(),
            ArgumentValue::RichText(rich_text) => rich_text.str(),
        }
    }

    pub fn into_str(self) -> CowStr<'a> {
        match self {
            ArgumentValue::Void => CowStr::Borrowed(""),
            ArgumentValue::PlainText(s) => s.clone(),
            ArgumentValue::RichText(rich_text) => rich_text.str().into_static(),
        }
    }

    pub fn parse<T: FromStr>(&'a self) -> Result<T, T::Err> {
        self.str().as_ref().parse()
    }

    pub fn rich_text(
        self, 
        preferred_representation: Option<RichTextRepresentation>
    ) -> RichText {
        let repr = preferred_representation.unwrap_or(RichTextRepresentation::CommonMark);
        match self {
            ArgumentValue::Void => 
                RichText::empty(repr),
            ArgumentValue::PlainText(s) => 
                RichText::from_str(s.as_ref(), repr),
            ArgumentValue::RichText(rich_text) => 
                rich_text,
        }
    }

    pub fn convert(
        self, 
        into: ValueType, 
        preferred_representation: Option<RichTextRepresentation>,
        location: Option<MinimalLocation>
    ) -> Result<ir::Value<'a>, ArgumentValueError> {
        match into {
            ValueType::Boolean => Ok(ir::Value::Boolean(true)),
            ValueType::String => Ok(ir::Value::String(self.into_str())),
            ValueType::RichText => Ok(ir::Value::RichText(self.rich_text(preferred_representation))),
            ValueType::SignedInteger => Ok(ir::Value::SignedInteger(
                self.parse()
                    .map_err(|e| ArgumentValueError::ParsingIntegerFailed(e, location))?
            )),
            ValueType::UnsignedInteger => Ok(ir::Value::UnsignedInteger(
                self.parse()
                    .map_err(|e| ArgumentValueError::ParsingIntegerFailed(e, location))?
            )),
            ValueType::URI => Ok(ir::Value::URI(
                self.parse()
                    .map_err(|e| ArgumentValueError::ParsingURIFailed(e, location))?
            )),
            ValueType::Link => {
                let rich_text = self.rich_text(preferred_representation);
                let (uri, title) = rich_text.link()?;
                Ok(ir::Value::Link(uri, title))
            }
            ValueType::Version => {
                Ok(ir::Value::Version(lenient_semver::parse(&self.str())
                    .map_err(|e: lenient_semver::parser::Error<'_>| ArgumentValueError::ParsingVersionFailed(
                        diagnostics::VersionError(e.error_kind(), e.erroneous_input().into()),
                        location.map(|l| l.within_offset_range(e.error_span()))
                    )
                )?))
            }
            ValueType::Void => {
                Ok(ir::Value::Void)
            }
        }
    }
}

impl<'a> Argument<'a> {
    pub fn convert(
        self, 
        value_type: ValueType, 
        preferred_representation: Option<RichTextRepresentation>,
    ) -> Result<ir::Value<'a>, ArgumentValueError> {
        self.value.convert(value_type, preferred_representation, Some(self.location))
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ArgumentValueError<ParsingError: std::error::Error = Infallible> {
    #[error("Missing argument value for parameter '{0}'")]
    Missing(String),

    #[error("Failed to parse value, but {0}")]
    ParsingFailed(ParsingError, Option<MinimalLocation>),

    #[error("Expected integer value, but {0}")]
    ParsingIntegerFailed(ParseIntError, Option<MinimalLocation>),

    #[error("Expected URI, but {0}")]
    ParsingURIFailed(url::ParseError, Option<MinimalLocation>),

    #[error("Expected version string, but {0}")]
    ParsingVersionFailed(diagnostics::VersionError, Option<MinimalLocation>),

    #[error(transparent)]
    LinkExtractionError(#[from] LinkExtractionError),
}

impl<E> diagnostics::Diagnostic<MinimalLocation> for ArgumentValueError<E> where E: std::error::Error {
    fn location(&self) -> Option<MinimalLocation> {
        match self {
            Self::ParsingFailed(_, location) =>
                location.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Arguments<'a>(Vec<Argument<'a>>);

impl<'a> Default for Arguments<'a> {
    fn default() -> Self {
        Self(vec![])
    }
}

impl<'a> Arguments<'a> {
    pub fn add(&mut self, name: &'a str, value: ArgumentValue<'a>, location: MinimalLocation) {
        self.0.push(Argument { name, value, location });
    }

    pub fn has(&self, name: &str) -> bool {
        self.0.iter().find(|a| a.name == name).is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Argument> {
        self.0.iter()
    }

    pub fn get<'c>(&self, name: &'c str) -> Option<&Argument> {
        self.0.iter().find(|a| a.name == name)
    }

    pub fn take<'c>(&mut self, name: &'c str) -> Option<Argument<'a>> {
        let ix = self.0.iter().enumerate().find(|(_, a)| a.name == name).map(|(ix, _)| ix)?;
        Some(self.0.swap_remove(ix))
    }

     fn materialize_template_component<'b: 'a, E: std::error::Error>(
        &'a self,
        component: &'a TemplateComponent<'b>,
    ) -> Result<CowStr<'a>, ArgumentValueError<E>> {
        match component {
                TemplateComponent::Literal(s) => Ok(s.clone()),
                TemplateComponent::Slot(name) => 
                    Ok(self.get(&name)
                        .ok_or_else(|| ArgumentValueError::Missing(name.into()))?
                        .value.str()
                    )
            }
    }

    fn materialize_template<'b: 'a, E: std::error::Error>(
        &'a self,
        template: &'a Template<'b>,
    ) -> Result<CowStr<'a>, ArgumentValueError<E>> {
        if template.components.is_empty() {
            return Ok(CowStr::Borrowed(""))
        }
        if template.components.len() == 1 {
            self.materialize_template_component(template.components.first().unwrap())
        } else {
            let mut out = String::with_capacity(template.capacity);
            for component in template.components.as_ref() {
                out.push_str(self.materialize_template_component(component)?.as_ref());
            }
            Ok(CowStr::Owned(out))
        }
    }

    
    fn materialize_str(
        &'a self,
        s: &'a str,
    ) -> Result<CowStr<'a>, ArgumentValueError> {
        let A: (usize, usize) = Template::_analyze(s, Template::SIGN);
        if A.0 == 1 && A.1 == 0 {
            return Ok(s.into())
        }
        let N: usize = A.0;
        let mut parts = Vec::with_capacity(N);
        Template::_parse_template(s, Template::SIGN, &mut parts);
        let template = Template::_from_components(parts.as_ref(), A.1);
        Ok(self.materialize_template(&template)?.into_static())
    }

    fn materialize_rich_text(
        &'a self,
        rich_text: &'a RichText,
    ) -> Result<RichText, ArgumentValueError> {
        rich_text.try_map_text(|s| Ok(self.materialize_str(s)?.into_static()))
    }

    pub fn resolve_str<T: AsRef<str> + Into<CowStr<'a>>>(
        &'a self, 
        parameter: &'a Parameter<T>
    ) -> Result<CowStr<'a>, ArgumentValueError> {
        match parameter {
            Parameter::Constant(s) => 
                Ok(CowStr::Borrowed(s.as_ref())),
            Parameter::Variable(name) => 
                Ok(self.get(&name)
                    .ok_or_else(|| ArgumentValueError::Missing(name.into()))?
                    .value.str()
                ),
            Parameter::Template(template) => 
                self.materialize_template(template),
            Parameter::Func(f) => 
                Ok(f(self).into()),
            Parameter::VariableDefault(name, default) =>
                Ok(self.get(&name)
                    .map(|a| a.value.str())
                    .unwrap_or(CowStr::Borrowed(default.as_ref()))
                )
        }
    }

    pub fn resolve_parsed<T: FromStr + Clone>(
        &'a self, 
        parameter: &'a Parameter<T>
    ) -> Result<T, ArgumentValueError<T::Err>> where T::Err: std::error::Error {
        match parameter {
            Parameter::Constant(s) => Ok(s.clone()),
            Parameter::Func(f) => Ok(f(self)),
            Parameter::Variable(name) => {
                let arg = self.get(&name)
                    .ok_or_else(|| ArgumentValueError::Missing(name.into()))?;

                    arg.value.parse()
                        .map_err(|e| ArgumentValueError::ParsingFailed(e, Some(arg.location.clone())))
            }
            Parameter::Template(template) => {
                self.materialize_template(template)?
                    .parse()
                    .map_err(|e| ArgumentValueError::ParsingFailed(e, None))
            }
            Parameter::VariableDefault(name, default) => {
                self.get(&name)
                    .map(|arg| {
                        arg.value.parse()
                            .map_err(|e| ArgumentValueError::ParsingFailed(e, Some(arg.location.clone())))
                    })
                    .unwrap_or(Ok(default.clone()))
            }
        }
    }

    pub fn resolve_rich_text(
        &'a self, 
        parameter: &'a Parameter<RichText>, 
        preferred_representation: Option<RichTextRepresentation>
    ) -> Result<RichText, ArgumentValueError> {
        match parameter {
            Parameter::Constant(s) => Ok(s.clone()),
            Parameter::Func(f) => Ok(f(self)),
            Parameter::Variable(name) => 
                Ok(self.get(&name)
                    .ok_or_else(|| ArgumentValueError::Missing(name.into()))?
                    .value.clone().rich_text(preferred_representation)),
            Parameter::Template(template) => 
                Ok(RichText::from_str(
                    self.materialize_template(&template)?.as_ref(), 
                    preferred_representation.unwrap_or(RichTextRepresentation::CommonMark))),
            Parameter::VariableDefault(name, default) =>
                Ok(self.get(&name)
                    .map(|arg| arg.value.clone().rich_text(preferred_representation))
                    .unwrap_or(default.clone())
                )
        }
    }

    pub fn resolve_rich_text_optional(
        &'a self, 
        parameter: &'a Parameter<Option<RichText>>, 
        preferred_representation: Option<RichTextRepresentation>
    ) -> Option<RichText> {
        match parameter {
            Parameter::Constant(s) => s.clone(),
            Parameter::Func(f) => f(self),
            Parameter::Variable(name) => 
                Some(self.get(&name)?.value.clone().rich_text(preferred_representation)),
            Parameter::Template(template) => 
                Some(RichText::from_str(
                    self.materialize_template::<Infallible>(&template).ok()?.as_ref(), 
                    preferred_representation.unwrap_or(RichTextRepresentation::CommonMark))),
            Parameter::VariableDefault(name, default) =>
                self.get(&name)
                    .map(|arg| arg.value.clone().rich_text(preferred_representation))
                    .or(default.clone())
        }
    }

    pub fn resolve_converted(
        &'a self, 
        parameter: &'a Parameter<ir::Value<'a>>,
        value_type: ir::ValueType
    ) -> Result<ir::Value<'a>, ArgumentValueError> {
        match parameter {
            Parameter::Constant(s) => Ok(s.clone()),
            Parameter::Func(f) => Ok(f(self)),
            Parameter::Variable(name) => 
                self.get(&name)
                    .ok_or_else(|| ArgumentValueError::Missing(name.into()))?
                    .clone()
                    .convert(value_type, None),
            Parameter::Template(template) => 
                ArgumentValue::<'a>::PlainText(self.materialize_template(template)?)
                    .convert(value_type, None, None),
            Parameter::VariableDefault(name, default) =>
                self.get(&name)
                    .map(|arg| arg.clone().convert(value_type, None))
                    .unwrap_or(Ok(default.clone()))
        }
    }

    pub fn resolve_converted_optional(
        &'a self, 
        parameter: &'a Parameter<Option<ir::Value<'a>>>,
        value_type: ir::ValueType
    ) -> Result<Option<ir::Value<'a>>, ArgumentValueError> {
        match parameter {
            Parameter::Constant(s) => Ok(s.clone()),
            Parameter::Func(f) => Ok(f(self)),
            Parameter::Variable(name) => 
                self.get(&name)
                    .map(|arg| arg.clone().convert(value_type, None))
                    .transpose(),
            Parameter::Template(template) => 
                self.materialize_template::<Infallible>(template).ok()
                    .map(|arg| ArgumentValue::<'a>::PlainText(arg).convert(value_type, None, None))
                    .transpose(),
            Parameter::VariableDefault(name, default) =>
                Ok(self.get(&name)
                    .map(|arg| arg.clone().convert(value_type, None))
                    .transpose()?
                    .or(default.clone()))
        }
    }
}
