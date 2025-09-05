use std::{any::Any, convert::Infallible, num::ParseIntError, ops::Index, path::PathBuf, str::{FromStr, ParseBoolError}};
use pulldown_cmark::{CowStr, InlineStr};
use strum_macros::AsRefStr;
use crate::{backend, compiler::{diagnostics::{self, EditorLocation, MinimalLocation}, URL}, ir::{self, rich_text::{LinkExtractionError, RichTextRepresentation}, Language, RichText, ValueType}};

#[derive(Clone, Debug)]
pub enum Parameter<T> {
    Variable(String), // parameter name
    // Templated(T, Vec<String>), // template, parameter names
    Constant(T),

    VariableDefault(String, T),
}

impl<T> Default for Parameter<T> where T: Default {
    fn default() -> Self {
        Self::Constant(T::default())
    }
}

impl<T> Parameter<T> {
    pub fn _Variable(variable: &'static str) -> Self {
        Self::Variable(variable.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct Argument<'a> {
    pub name: &'a str,
    pub value: ArgumentValue<'a>,
    pub location: MinimalLocation,
}

#[derive(Debug, Clone)]
pub enum ArgumentValue<'a> {
    Void,

    PlainText(&'a str),

    RichText(RichText),
}

impl<'a> ArgumentValue<'a> {
    pub fn from_opt_str(s: Option<&'a str>) -> Self {
        s.map_or(Self::Void, |s| Self::PlainText(s))
    }

    pub fn str(&'a self) -> CowStr<'a> {
        match self {
            ArgumentValue::Void => CowStr::Borrowed(""),
            ArgumentValue::PlainText(s) => CowStr::Borrowed(s),
            ArgumentValue::RichText(rich_text) => rich_text.str(),
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
                RichText::from_str(s, repr),
            ArgumentValue::RichText(rich_text) => 
                rich_text,
        }
    }
}

impl<'a> Argument<'a> {
        pub fn convert(
        self, 
        value_type: ValueType, 
        preferred_representation: Option<RichTextRepresentation>,
    ) -> Result<ir::Value, ArgumentValueError> {
        match value_type {
            ValueType::Boolean => Ok(ir::Value::Boolean(true)),
            ValueType::String => Ok(ir::Value::String(self.value.str().into())),
            ValueType::RichText => Ok(ir::Value::RichText(self.value.rich_text(preferred_representation))),
            ValueType::SignedInteger => Ok(ir::Value::SignedInteger(
                self.value.parse()
                    .map_err(|e| ArgumentValueError::ParsingIntegerFailed(e, self.location))?
            )),
            ValueType::UnsignedInteger => Ok(ir::Value::UnsignedInteger(
                self.value.parse()
                    .map_err(|e| ArgumentValueError::ParsingIntegerFailed(e, self.location))?
            )),
            ValueType::URI => Ok(ir::Value::URI(
                self.value.parse()
                    .map_err(|e| ArgumentValueError::ParsingURIFailed(e, self.location))?
            )),
            ValueType::Link => {
                let rich_text = self.value.rich_text(preferred_representation);
                let (uri, title) = rich_text.link()?;
                Ok(ir::Value::Link(uri, title.into()))
            }
            ValueType::Version => {
                Ok(ir::Value::Version(lenient_semver::parse(&self.value.str())
                    .map_err(|e| ArgumentValueError::ParsingVersionFailed(
                        diagnostics::VersionError(e.error_kind(), e.erroneous_input().into()),
                        self.location.within_offset_range(e.error_span())
                    )
                )?))
            }
        }
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ArgumentValueError<ParsingError: std::error::Error = Infallible> {
    #[error("Missing argument value for parameter '{0}'")]
    Missing(String),

    #[error("Failed to parse value, but {0}")]
    ParsingFailed(ParsingError, MinimalLocation),

    #[error("Expected integer value, but {0}")]
    ParsingIntegerFailed(ParseIntError, MinimalLocation),

    #[error("Expected URI, but {0}")]
    ParsingURIFailed(url::ParseError, MinimalLocation),

    #[error("Expected version string, but {0}")]
    ParsingVersionFailed(diagnostics::VersionError, MinimalLocation),

    #[error(transparent)]
    LinkExtractionError(#[from] LinkExtractionError),
}

impl<E> diagnostics::Diagnostic<MinimalLocation> for ArgumentValueError<E> where E: std::error::Error {
    fn location(&self) -> Option<MinimalLocation> {
        match self {
            Self::ParsingFailed(_, location) =>
                Some(location.clone()),
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

    pub fn take<'c>(&mut self, name: &'c str) -> Option<Argument> {
        let ix = self.0.iter().enumerate().find(|(_, a)| a.name == name).map(|(ix, _)| ix)?;
        Some(self.0.swap_remove(ix))
    }

    pub fn resolve_str<T: AsRef<str>>(
        &'a self, 
        parameter: &'a Parameter<T>
    ) -> Result<CowStr<'a>, ArgumentValueError> {
        match parameter {
            Parameter::Constant(s) => 
                Ok(CowStr::Borrowed(s.as_ref())),
            Parameter::Variable(name) => 
                Ok(self.get(&name)
                    .ok_or_else(|| ArgumentValueError::Missing(name.clone()))?
                    .value.str()
                ),
            Parameter::VariableDefault(name, default) =>
                Ok(self.get(&name)
                    .map(|a| a.value.str())
                    .unwrap_or(CowStr::Borrowed(default.as_ref()))
                )
        }
    }

    pub fn resolve_parsed<T: FromStr>(
        &'a self, 
        parameter: Parameter<T>
    ) -> Result<T, ArgumentValueError<T::Err>> where T::Err: std::error::Error {
        match parameter {
            Parameter::Constant(s) => Ok(s),
            Parameter::Variable(name) => {
                let arg = self.get(&name)
                    .ok_or_else(|| ArgumentValueError::Missing(name))?;

                    arg.value.parse()
                        .map_err(|e| ArgumentValueError::ParsingFailed(e, arg.location.clone()))
            }
            Parameter::VariableDefault(name, default) => {
                self.get(&name)
                    .map(|arg| {
                        arg.value.parse()
                            .map_err(|e| ArgumentValueError::ParsingFailed(e, arg.location.clone()))
                    })
                    .unwrap_or(Ok(default))
            }
        }
    }

    pub fn resolve_rich_text(
        &'a mut self, 
        parameter: Parameter<RichText>, 
        preferred_representation: Option<RichTextRepresentation>
    ) -> Result<RichText, ArgumentValueError> {
        match parameter {
            Parameter::Constant(s) => Ok(s),
            Parameter::Variable(name) => 
                Ok(self.take(&name)
                    .ok_or_else(|| ArgumentValueError::Missing(name))?
                    .value.rich_text(preferred_representation)),
            Parameter::VariableDefault(name, default) =>
                Ok(self.take(&name)
                    .map(|arg| arg.value.rich_text(preferred_representation))
                    .unwrap_or(default)
                )
        }
    }

    pub fn resolve_rich_text_optional(
        &mut self, 
        parameter: Parameter<Option<RichText>>, 
        preferred_representation: Option<RichTextRepresentation>
    ) -> Option<RichText> {
        match parameter {
            Parameter::Constant(s) => s,
            Parameter::Variable(name) => 
                Some(self.take(&name)?.value.rich_text(preferred_representation)),
            Parameter::VariableDefault(name, default) =>
                self.take(&name)
                    .map(|arg| arg.value.rich_text(preferred_representation))
                    .or(default)
        }
    }

    pub fn resolve_converted(
        &mut self, 
        parameter: Parameter<ir::Value>,
        value_type: ir::ValueType
    ) -> Result<ir::Value, ArgumentValueError> {
        match parameter {
            Parameter::Constant(s) => Ok(s),
            Parameter::Variable(name) => 
                self.take(&name)
                    .ok_or_else(|| ArgumentValueError::Missing(name))?
                    .convert(value_type, None),
            Parameter::VariableDefault(name, default) =>
                self.take(&name)
                    .map(|arg| arg.convert(value_type, None))
                    .unwrap_or(Ok(default))
        }
    }

    pub fn resolve_converted_optional(
        &'a mut self, 
        parameter: Parameter<Option<ir::Value>>,
        value_type: ir::ValueType
    ) -> Result<Option<ir::Value>, ArgumentValueError> {
        match parameter {
            Parameter::Constant(s) => Ok(s),
            Parameter::Variable(name) => 
                self.take(&name)
                    .map(|arg| arg.convert(value_type, None))
                    .transpose(),
            Parameter::VariableDefault(name, default) =>
                Ok(self.take(&name)
                    .map(|arg| arg.convert(value_type, None))
                    .transpose()?
                    .or(default))
        }
    }
}
