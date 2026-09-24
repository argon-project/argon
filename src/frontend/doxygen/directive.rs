use std::collections::HashMap;

use crate::{frontend, ir};

#[derive(Clone, Debug)]
/// Comma-separated modifier arguments
pub enum DoxygenDelimitedParameters<'a> {
    None,

    /// Predefined keywords in square brackets
    /// 
    /// ```
    /// @param[in,out] blah blah
    /// ```
    KeywordModifiers(&'a[&'a str]),

    /// Exactly the number of specified arguments in curly braces
    /// 
    /// ```
    /// @customcommand{myarg0,thesecondarg,"anotherargument"}
    /// ```
    /// 
    /// This is the syntax used by custom-defined doxygen directives
    PositionalArguments(&'a[&'a str], usize, usize), // names, mandatory, optional

    /// Keywords with optionally assigned values (bool) using separator (char) in curly braces
    /// 
    /// ```
    /// @lineinfo{extension}
    /// @cite{shortauthor,nopar,nocite}
    /// @tableofcontents{HTML:3,LaTeX,DocBook:1}
    /// @include{doc,raise=1}
    /// ```
    Options(char, HashMap<&'a str, bool>),
}

impl<'a> DoxygenDelimitedParameters<'a> {
    pub fn delimiters(&self) -> Option<(u8, u8)> {
        match self {
            Self::None => None,
            Self::KeywordModifiers(_) => Some((b'[', b']')),
            Self::PositionalArguments(_,_,_) | Self::Options(_,_) => Some((b'{', b'}'))
        }
    }
}

#[derive(Debug, Clone)]
pub struct DoxygenWhitespaceSeparatedParameters<'a> {
    pub required: &'a [&'a str],
    pub optional: &'a [&'a str],
}

#[derive(Debug, Clone)]
pub struct DoxygenAttachedParameter<'a> {
    pub name: &'a str,
    pub required: bool,
}

pub type DoxygenArgumentValidator = fn(&[&str], &[&str]) -> Result<(), frontend::doxygen::DoxygenError>;

#[derive(Debug, Clone)]
pub struct DoxygenParameters<'a> {
    pub delimited: DoxygenDelimitedParameters<'a>,
    pub whitespace_separated: DoxygenWhitespaceSeparatedParameters<'a>,
    pub attached: Option<DoxygenAttachedParameter<'a>>,
    pub validator: Option<DoxygenArgumentValidator>,
}

#[derive(Debug, Clone)]
pub struct DoxygenDirective<'a> {
    /// A function that validates delimited and whitespace-separated arguments
    pub parameters: DoxygenParameters<'a>,
    pub syntax: pulldown_cmark::doc_tags::Syntax,
    pub script: frontend::actions::Script<'a, DoxygenBuiltin>,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum DoxygenBuiltin {
    FileInfo,
    LineInfo,
    File,
    Doxyconfig,
}