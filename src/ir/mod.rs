pub(crate) mod rich_text;
pub mod entry_graph;

pub use entry_graph::{
    Symbol,
    RoleID,
    Attribute,
    Role,
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
};

pub use rich_text::{
    RichText,
};
use serde::{
    Deserialize, Serialize
};
use strum_macros::Display;  

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
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Display, Deserialize, Serialize)]
pub enum Language {
    #[serde(rename = "C", alias = "c")]
    C = 1,
    CPlusPlus = 2,
    Swift = 3,
    Rust = 4,
    Java = 5,
    JavaScript = 6,
    JSON = 7,
}