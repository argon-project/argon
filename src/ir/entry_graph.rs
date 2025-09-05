#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use bimap::BiMap;
use petgraph::graph::DiGraph;
use petgraph::prelude::DiGraphMap;
use semver::{Op, Version};
use strum_macros::{
    EnumDiscriminants,
};

use crate::compiler::URL;
use crate::driver;
use crate::ir::rich_text::{ RichText };
use crate::frontend::c;
use crate::ir::Language;

#[derive(Clone, Debug)]
pub struct SymbolKind {
    id: String,
    highlighting_language: tree_sitter::Language,
    likeness: SymbolLikeness,
}

#[derive(Clone)]
pub enum Symbol {
    c(c::sema::Symbol),
    raw(SymbolKind, String)
}

impl Symbol {
    pub fn likeness(&self) -> SymbolLikeness {
        match self {
            Self::c(symbol) => symbol.likeness(),
            Self::raw(kind, _) => kind.likeness.clone(),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Debug)]
pub enum SymbolLikeness {
    Function = 1,
    Variable = 2,
    Constant = 3,
    Structure = 4,
    Enumeration = 5,
    Statement = 6,
}

#[repr(u8)]
#[derive(Clone, Debug)]
pub enum ArtifactLikeness {
    GenericFile = 0,
    Source = 1,
    Header = 2,
    HumanReadableData = 3,
}

/// Role Identifier
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct RoleID(u8, u8, u8);

impl RoleID {
    pub const group: RoleID = RoleID(0, 0, 0);

    pub fn is_member_allowed(&self, member_role: RoleID) -> bool {
        true
    }

    pub const fn symbol(language: Language, likeness: SymbolLikeness) -> RoleID {
        RoleID(likeness as u8, language as u8, 1)
    }

    pub const fn artifact(language: Language, likeness: ArtifactLikeness) -> RoleID {
        RoleID(likeness as u8, language as u8, 2)
    }

    pub fn from_extension(extension: &str) -> RoleID {
        let Some((language, likeness)) = language_artifact_from_extension(extension) else {
            return RoleID(ArtifactLikeness::GenericFile as u8, 0, 2)
        };
        Self::artifact(language, likeness)
    }
}

pub fn language_artifact_from_extension(extension: &str) -> Option<(Language, ArtifactLikeness)> {
    match extension {
        "h" => Some((Language::C, ArtifactLikeness::Header)),
        "c" => Some((Language::C, ArtifactLikeness::Source)),
        "hpp" => Some((Language::CPlusPlus, ArtifactLikeness::Header)),
        "cpp" => Some((Language::CPlusPlus, ArtifactLikeness::Source)),
        "swift" => Some((Language::CPlusPlus, ArtifactLikeness::Source)),
        "rs" => Some((Language::Rust, ArtifactLikeness::Source)),
        "js" => Some((Language::JavaScript, ArtifactLikeness::Source)),
        "java" => Some((Language::Java, ArtifactLikeness::Source)),
        "json" => Some((Language::JSON, ArtifactLikeness::HumanReadableData)),
        _ => None
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum RoleLookupError {
    
    #[error("Unknown role '{0}'")]
    UnknownRoleKey(String),
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum AttributeLookupError {
    
    #[error("Unknown attribute '{0}'")]
    UnknownAttributeKey(String),
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum EntryLookupError {
    
    #[error("Unknown entry '{0}'")]
    UnknownEntryID(String),
}

#[derive(Clone, Debug, EnumDiscriminants)]
#[strum_discriminants(name(ValueType))]
pub enum Value {
    /// Rich-text value
    RichText(RichText),

    /// String value
    String(String),

    /// URI, e.g. mailto link
    Link(URL, String),

    URI(URL),

    /// Integer value
    SignedInteger(isize),

    /// Integer value
    UnsignedInteger(usize),

    Boolean(bool),

    Version(Version),
}

#[derive(Clone)]
pub struct MemberSection {
    pub label: String,
    
    pub description: Option<RichText>,

    members: Vec<String>
}

#[derive(Clone, Debug)]
pub struct CharacteristicKind(String, String);

#[derive(Clone, Debug)]
pub enum CharacteristicSection {
    Parameters,
    ReturnValue,
    ThrownError,
    Custom(CharacteristicKind)
}

#[derive(Clone, Debug)]
pub struct Modifier(String);

pub enum CharacteristicSectionListKind {
    DefinitionList,
    UnorderedList,
    OrderedList
}

pub struct CharacteristicModifier(String);

#[derive(Clone)]
pub struct Entry {
    pub role: RoleID,

    pub title: String,

    pub abstract_: Option<RichText>,

    pub description: Option<RichText>,

    pub attributes: Vec<(String, Value)>,

    pub symbols: Vec<Symbol>,

    pub members: Vec<String>,

    pub characterstic_sections: Vec<CharacteristicSection>,

    pub member_sections: Vec<MemberSection>,
}

impl Entry {
    pub fn named(title: String) -> Self {
        Self {
            role: RoleID::group,
            title: title,
            abstract_: None,
            description: None,
            attributes: vec![],
            symbols: vec![],
            characterstic_sections: vec![],
            member_sections: vec![],
        }
    }

    pub fn assign_role(&mut self, role: RoleID) {
        self.role = role
    }

    pub fn get_attribute_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.attributes.iter_mut().find(|(a,_)| *a == key).map(|(_, v)| v)
    }

    pub fn get_attribute(&self, key: &str) -> Option<Value> {
        self.attributes.iter().find(|(a,_)| *a == key).map(|(_, v)| v.clone())
    }

    pub fn get_attributes(&self, key: &str) -> Vec<Value> {
        self.attributes.iter().filter(|(a,_)| *a == key).map(|(_, v)| v.clone()).collect()
    }

    pub fn get_attributes_mut(&mut self, key: &str) -> Vec<&mut Value> {
        self.attributes.iter_mut().filter(|(a,_)| *a == key).map(|(_, v)| v).collect()
    }

    pub fn set_attribute(&mut self, key: String, value: Value) {
        if let Some(existing_value) = self.get_attribute_mut(&key) {
            *existing_value = value;
        } else {
            self.attributes.push((key, value));
        }
    }

    pub fn set_abstract(&mut self, abstract_: RichText) {
        self.abstract_= Some(abstract_)
    }

    pub fn add_attribute(&mut self, key: String, value: Value) {
        self.attributes.push((key, value));
    }

    pub fn can_add_member(&self, role: RoleID) -> bool {
        self.role.is_member_allowed(role)
    }

    pub fn add_member(&mut self, id: String) {
        self.members.push(id);
    }
}


pub struct Role {
    pub label: String,
    pub id: RoleID
}

impl Role {
    pub fn id(&self) -> RoleID {
        self.id.clone()
    }
}

pub struct Attribute {
    pub label: String,
    pub repeatable: bool,
    pub value_type: ValueType,
    pub description: Option<RichText>
}

pub struct Roles {
    storage: HashMap<RoleID, Role>,
    map: BiMap<String, RoleID>,
}

impl Default for Roles {
    fn default() -> Self {
        Self {
            storage: HashMap::new(),
            map: BiMap::new(),
        }
    }
}

impl From<Vec<crate::driver::manifest::v1::DocRole>> for Roles {
    fn from(roles: Vec<crate::driver::manifest::v1::DocRole>) -> Self {
        Self::from_manifest_v1(roles)
    }
}

impl Roles {

    pub fn from_manifest_v1(roles: Vec<crate::driver::manifest::v1::DocRole>) -> Self {
        let mut s = Self::default();
        for (ix, role) in roles.iter().enumerate() {
            let id = RoleID(0, 0, 2 + ix as u8);
            s.map.insert(role.id.clone(), id.clone());
            s.storage.insert(id.clone(), Role { 
                label: role.label.clone(), 
                id: id
            });
        }
        s
    }

    pub fn get_by_key(&self, key: &str) -> Option<&Role> {
        self.map
            .get_by_left(key)
            .map(|id| self.storage.get(id))
            .flatten()
    }

    pub fn get_by_id(&self, id: RoleID) -> Option<&Role> {
        self.storage.get(&id)
    }

    pub fn id_for_key(&self, key: &str) -> Option<RoleID> {
        Some(self.map.get_by_left(key)?.clone())
    }

    pub fn key_for_id(&self, id: RoleID) -> Option<&str> {
        Some(self.map.get_by_right(&id)?.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Relationship {
    Member
}

pub struct EntryGraph {
    pub roles: Roles,
    pub attributes: HashMap<String, Attribute>,
    entries: HashMap<String, Entry>,
    relationships: DiGraph<String, Relationship>,
    root: String,
}

impl EntryGraph {
    pub fn new() -> Self {
        Self {
            roles: Roles::default(),
            attributes: HashMap::new(),
            entries: HashMap::with_capacity(DEFAULT_ENTRY_CAPACITY),
            root: "root.default".into()
        }
    }

    pub fn set_root(&mut self, id: String) {
        assert!(self.entries.contains_key(&id));
        self.root = id;
    }

    pub fn add_entry<'a>(&'a mut self, id: &str, title: String) -> &'a mut Entry {
        self.entries.insert(id.into(), Entry::named(title));
        self.entries.get_mut(id).expect("fatal error: Entry ID not found")
    }

    pub fn get_entry<'a>(&'a mut self, id: &str) -> Option<&'a mut Entry> {
        self.entries.get_mut(id)
    }

}

#[derive(Clone)]
pub enum EntryGraphError {

}

const DEFAULT_ENTRY_CAPACITY: usize = 1000;