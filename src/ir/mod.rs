pub(crate) mod rich_text;
pub mod entry_graph;

pub use entry_graph::{
    Symbol,
    RoleID,
    Attribute,
    Attributes,
    Role,
    Roles,
    MemberSection,
    CharacteristicSection,
    Entry,
    EntryGraph,
    Modifier,
    SymbolLikeness,
    SymbolKind,
    ArtifactLikeness,
    Value,
    ValueType,
    // Aspect,
    // AspectID,
    AttributeAppearance,
    InliningBehavior,
    Variable,
    Relationship,
    // AspectItem
};

pub use rich_text::{
    RichText,
    AdmonitionKind,
};
use serde::{
    Deserialize, Serialize
};
use strum_macros::{Display, EnumString};  

#[repr(u8)]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Display, Deserialize, Serialize)]
pub enum Dialect {
    #[serde(rename = "doxygen")]
    Doxygen = 1,

    #[serde(rename = "docc")]
    DocC = 2,

    #[serde(rename = "rustdoc")]
    RustDoc = 3,
}

#[repr(u8)]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Display, Deserialize, Serialize, EnumString)]
pub enum Language {
    #[serde(rename = "C", alias = "c")]
    #[strum(serialize = "c", serialize = "C")]
    C = 1,

    #[strum(serialize = "C++", serialize = "c++", serialize = "CPP", serialize = "cpp")]
    CPlusPlus = 2,

    #[strum(serialize = "Swift", serialize = "swift")]
    Swift = 3,

    #[strum(serialize = "Rust", serialize = "rust", serialize = "rs")]
    Rust = 4,

    #[strum(serialize = "Java", serialize = "java")]
    Java = 5,

    #[strum(serialize = "JS", serialize = "js", serialize = "JavaScript", serialize = "javascript")]
    JavaScript = 6,

    #[strum(serialize = "JSON", serialize = "json")]
    JSON = 7,

    #[strum(serialize = "XML", serialize = "xml")]
    XML = 8,

    #[strum(serialize = "HTML", serialize = "html")]
    HTML = 9,

    #[strum(serialize = "RTF", serialize = "rtf")]
    RTF = 10,

    #[strum(serialize = "LaTeX", serialize = "latex")]
    LaTeX = 11,

    #[strum(serialize = "Man", serialize = "man")]
    ManPages = 12,

    #[strum(serialize = "DocBook", serialize = "docbook")]
    DocBook = 13,

    #[strum(serialize = "Markdown", serialize = "md")]
    Markdown = 14,
}

#[repr(u8)]
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Display, Deserialize, Serialize, EnumString)]
pub enum DiagnosticLevel {
    #[strum(serialize = "help")]
    Help,

    #[strum(serialize = "trace")]
    Trace,

    #[strum(serialize = "error")]
    Error,

    #[strum(serialize = "warning")]
    Warning,
}