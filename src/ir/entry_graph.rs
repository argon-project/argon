#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::Infallible;
use std::hash::Hash;
use std::mem;
use std::ops::Range;
use std::path::PathBuf;
use std::str::FromStr;

use bimap::BiMap;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::prelude::DiGraphMap;
use petgraph::Direction::{Incoming, Outgoing};
use semver::{Op, Version};
use serde::{Deserialize, Serialize};
use strum_macros::{
    EnumDiscriminants,
};

use crate::compiler::URL;
use crate::compiler::strings::CowStr;
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

#[derive(Clone, Debug)]
pub struct ExpressionKind {
    highlighting_language: tree_sitter::Language,
}

#[derive(Clone)]
pub enum Expression {
    c(c::sema::Expression),
    raw(ExpressionKind, String)
}

pub trait Variable {
    fn nullable(&self) -> Option<bool> { None }
    fn optional(&self) -> bool { false }
    fn default(&self) -> Option<Expression> { None }
    fn variadic(&self) -> bool { false }
    fn pointerlike(&self) -> bool { false }
    fn identifier(&self) -> Option<&str>;
    fn constant(&self) -> bool { false }
}

impl Symbol {
    pub fn likeness(&self) -> SymbolLikeness {
        match self {
            Self::c(symbol) => symbol.likeness(),
            Self::raw(kind, _) => kind.likeness.clone(),
        }
    }

    pub fn parameters<'a>(&'a self) -> Box<dyn Iterator<Item = &'a impl Variable> + 'a> {
        match self {
            Self::c(c::sema::Symbol::function(func)) => Box::new(func.parameters.iter()),
            _ => Box::new(std::iter::empty()),
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
    EnumerationCase = 6,
    Statement = 7,
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
pub enum Value<'a> {
    /// Rich-text value
    RichText(RichText),

    /// String value
    String(CowStr<'a>),

    Void,
    
    /// URI, e.g. mailto link
    Link(URL, CowStr<'a>),

    URI(URL),
    
    /// Integer value
    SignedInteger(isize),

    /// Integer value
    UnsignedInteger(usize),

    Boolean(bool),

    Version(Version),
}

impl<'a, T> From<T> for Value<'a> where T: Into<CowStr<'a>> {
    fn from(value: T) -> Self {
        Self::String(value.into())
    }
}

impl<'a> Value<'a> {
    pub fn into_static(self) -> Value<'static> {
        match self {
            Self::String(s) => Value::String(s.into_static()),
            Self::Link(url, s) => Value::Link(url, s.into_static()),
            a => unsafe { std::mem::transmute(a) }
        }
    }
}

#[derive(Clone)]
pub struct MemberSection {
    pub title: String,
    
    pub description: Option<RichText>,

    members: Vec<String>
}

#[derive(Clone, Debug)]
pub struct CharacteristicSection(Cow<'static, str>);


impl CharacteristicSection {
    pub const fn predefined(identifier: &'static str) -> Self {
        Self(Cow::Borrowed(identifier))
    }

    pub const PARAMETERS: CharacteristicSection = CharacteristicSection::predefined("arg");
    pub const RETURNS: CharacteristicSection = CharacteristicSection::predefined("ret");
    pub const THROWS: CharacteristicSection = CharacteristicSection::predefined("err");
    pub const PRECONDITION: CharacteristicSection = CharacteristicSection::predefined("c.pre");
    pub const POSTCONDITION: CharacteristicSection = CharacteristicSection::predefined("c.post");
    pub const ISSUES: CharacteristicSection = CharacteristicSection::predefined("issue");
}

impl FromStr for CharacteristicSection {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Cow::Owned(s.to_owned())))
    }
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
    graph_id: Option<NodeIndex>,

    pub role: RoleID,

    pub title: String,

    pub abstract_: Option<RichText>,

    pub description: Option<RichText>,

    pub attributes: AttributeValues<'static>,

    pub symbols: Vec<Symbol>,

    // pub aspects: AspectItems,

    pub member_sections: Vec<MemberSection>,
}

impl Entry {
    pub fn named(title: String) -> Self {
        Self {
            graph_id: None,
            role: RoleID::group,
            title: title,
            abstract_: None,
            description: None,
            attributes: AttributeValues::default(),
            symbols: vec![],
            // aspects: AspectItems::default(),
            member_sections: vec![],
        }
    }

    pub fn assign_role(&mut self, role: RoleID) {
        self.role = role
    }

    pub fn can_add_member(&self, role: RoleID) -> bool {
        self.role.is_member_allowed(role)
    }

    pub fn add_member_section(&mut self, title: String, description: Option<RichText>) -> &mut MemberSection {
        self.member_sections.push(MemberSection { title, description, members: vec![] });
        self.member_sections.last_mut().unwrap()
    }

    pub fn add_symbols(&mut self, symbols: Vec<Symbol>) {
        if self.symbols.is_empty() {
            self.symbols = symbols
        } else {
            let mut symbols = symbols;
            self.symbols.append(&mut symbols);
        }
    }

    pub fn set_abstract(&mut self, abstract_: RichText) {
        self.abstract_= Some(abstract_)
    }

}

#[derive(Clone, Debug)]
pub struct InliningBehavior {
    pub ordered: bool
}


#[derive(Clone, Debug)]
pub struct Role {
    pub label: String,
    pub inlining_behavior: Option<InliningBehavior>
}

impl Role {
    pub fn inline(&self) -> bool {
        self.inlining_behavior.is_some()
    }
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

impl<I: Iterator<Item = (String, Role)>> From<I> for Roles {
    fn from(roles: I) -> Self {
       let mut s = Self::default();
        for (ix, (key, role)) in roles.into_iter().enumerate() {
            let id = RoleID(0, 0, 2 + ix as u8);
            s.map.insert(key, id);
            s.storage.insert(id.clone(), Role { 
                label: role.label, 
                inlining_behavior: role.inlining_behavior.map(|b| b.into())
            });
        }
        s
    }
}

impl Roles {
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

#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub struct Relationship(Cow<'static, str>);


impl Relationship {
    pub const fn predefined(identifier: &'static str) -> Self {
        Self(Cow::Borrowed(identifier))
    }

    pub const Owner: Self = Self::predefined("owns");
    pub const Member: Self = Self::predefined("member");
    pub const Related: Self = Self::predefined("rel");
}

impl FromStr for Relationship {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Cow::Owned(s.to_owned())))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeAppearance {
    Modifier,
    Annotation,
    Caption,
    Admonition,
    Hidden,
}

#[derive(Debug, Clone)]
pub struct Attribute {
    pub label: String,
    pub repeatable: bool,
    pub value_type: ValueType,
    pub appearance: AttributeAppearance,
    pub description: Option<RichText>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttributeID(usize);

#[derive(Clone, Debug, Default)]
pub struct AttributeValues<'a>(Vec<(AttributeID, Value<'a>)>);

impl<'a> AttributeValues<'a> {
    pub fn get_attribute_mut(&mut self, id: AttributeID) -> Option<&mut Value<'a>> {
        self.0.iter_mut().find(|(a,_)| *a == id).map(|(_, v)| v)
    }

    pub fn get_attribute(&self, id: AttributeID) -> Option<Value<'a>> {
        self.0.iter().find(|(a,_)| *a == id).map(|(_, v)| v.clone())
    }

    pub fn get_attributes(&self, id: AttributeID) -> Vec<Value<'a>> {
        self.0.iter().filter(|(a,_)| *a == id).map(|(_, v)| v.clone()).collect()
    }

    pub fn get_attributes_mut(&mut self, id: AttributeID) -> Vec<&mut Value<'a>> {
        self.0.iter_mut().filter(|(a,_)| *a == id).map(|(_, v)| v).collect()
    }

    pub fn set_attribute(&mut self, id: AttributeID, value: Value<'a>) {
        if let Some(existing_value) = self.get_attribute_mut(id) {
            *existing_value = value;
        } else {
            self.0.push((id, value));
        }
    }

    pub fn add_attribute(&mut self, id: AttributeID, value: Value<'a>) {
        self.0.push((id, value));
    }
}

#[derive(Clone, Debug)]
pub struct Attributes {
    storage: Vec<Attribute>,
    map: BiMap<String, AttributeID>,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            storage: Vec::new(),
            map: BiMap::new(),
        }
    }
}

impl<I: Iterator<Item = (String, Attribute)>> From<I> for Attributes {
    fn from(attributes: I) -> Self {
        let mut s = Self::default();
        for (ix, (key, attr)) in attributes.enumerate() {
            let id = AttributeID(ix);
            s.map.insert(key, id);
            s.storage.push(attr.into());
        }
        s
    }
}

impl Attributes {
    pub fn get_by_key(&self, key: &str) -> Option<&Attribute> {
        self.map
            .get_by_left(key)
            .map(|id| self.storage.get(id.0))
            .flatten()
    }

    pub fn get_by_id(&self, id: AttributeID) -> Option<&Attribute> {
        self.storage.get(id.0)
    }

    pub fn id_for_key(&self, key: &str) -> Option<AttributeID> {
        Some(self.map.get_by_left(key)?.clone())
    }

    pub fn key_for_id(&self, id: AttributeID) -> Option<&str> {
        Some(self.map.get_by_right(&id)?.as_str())
    }
}

// #[derive(Debug, Clone)]
// pub struct Aspect {
//     pub label: String,
//     pub help: Option<RichText>,
//     pub items_ordered: bool,
//     pub items_labelled: bool,
//     pub items_referenced: bool,
// }

// #[derive(Debug, Clone)]
// pub struct AspectItem {
//     pub label: Option<String>,
//     pub reference: Option<String>,
//     pub attributes: AttributeValues,
//     pub description: RichText,
// }

// #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// pub struct AspectID(usize);

// #[derive(Clone, Debug, Default)]
// pub struct AspectItems(HashMap<AspectID, Vec<AspectItem>>);

// impl AspectItems {
//     pub fn get_attribute_mut(&mut self, id: AttributeID) -> Option<&mut Value> {
//         self.0.iter_mut().find(|(a,_)| *a == id).map(|(_, v)| v)
//     }

//     pub fn get_attribute(&self, id: AttributeID) -> Option<Value> {
//         self.0.iter().find(|(a,_)| *a == id).map(|(_, v)| v.clone())
//     }

//     pub fn get_attributes(&self, id: AttributeID) -> Vec<Value> {
//         self.0.iter().filter(|(a,_)| *a == id).map(|(_, v)| v.clone()).collect()
//     }

//     pub fn get_attributes_mut(&mut self, id: AttributeID) -> Vec<&mut Value> {
//         self.0.iter_mut().filter(|(a,_)| *a == id).map(|(_, v)| v).collect()
//     }

//     pub fn set_attribute(&mut self, id: AttributeID, value: Value) {
//         if let Some(existing_value) = self.get_attribute_mut(id) {
//             *existing_value = value;
//         } else {
//             self.0.push((id, value));
//         }
//     }

//     pub fn add_attribute(&mut self, id: AttributeID, value: Value) {
//         self.0.push((id, value));
//     }
// }

// #[derive(Clone, Debug)]
// pub struct Aspects {
//     storage: Vec<Aspect>,
//     map: BiMap<String, AspectID>,
// }

// impl Default for Aspects {
//     fn default() -> Self {
//         Self {
//             storage: Vec::new(),
//             map: BiMap::new(),
//         }
//     }
// }

// impl From<Vec<crate::driver::manifest::v1::DocAspect>> for Aspects {
//     fn from(aspects: Vec<crate::driver::manifest::v1::DocAspect>) -> Self {
//         Self::from_manifest_v1(aspects)
//     }
// }

// impl Aspects {
//     pub fn from_manifest_v1(attributes: Vec<crate::driver::manifest::v1::DocAspect>) -> Self {
//         let mut s = Self::default();
//         for (ix, aspect) in attributes.into_iter().enumerate() {
//             let id = AspectID(ix);
//             s.map.insert(aspect.id.clone(), id);
//             s.storage.push(aspect.into());
//         }
//         s
//     }

//     pub fn get_by_key(&self, key: &str) -> Option<&Aspect> {
//         self.map
//             .get_by_left(key)
//             .map(|id| self.storage.get(id.0))
//             .flatten()
//     }

//     pub fn get_by_id(&self, id: AspectID) -> Option<&Aspect> {
//         self.storage.get(id.0)
//     }

//     pub fn id_for_key(&self, key: &str) -> Option<AspectID> {
//         Some(self.map.get_by_left(key)?.clone())
//     }

//     pub fn key_for_id(&self, id: AspectID) -> Option<&str> {
//         Some(self.map.get_by_right(&id)?.as_str())
//     }
// }

pub struct EntryGraph {
    pub roles: Roles,
    pub attributes: Attributes,
    // pub aspects: Aspects,
    entries: HashMap<String, (Entry, NodeIndex)>,
    relationships: DiGraph<String, Relationship>,
    root: String,
}

impl EntryGraph {
    fn root_id(&self) -> &str {
        &self.root
    }

    fn root_mut(&mut self) -> &mut Entry {
        self.get_entry_mut(&self.root_id().to_string()).expect("fatal error: root entry not found")
    }

    fn root(&mut self) -> &Entry {
        self.get_entry(self.root_id()).expect("fatal error: root entry not found")
    }

    pub fn new(roles: Roles, attributes: Attributes) -> Self {
        Self {
            roles,
            attributes,
            // aspects: Aspects::default(),
            relationships: DiGraph::new(),
            entries: HashMap::with_capacity(DEFAULT_ENTRY_CAPACITY),
            root: "root.default".into()
        }
    }

    pub fn set_root(&mut self, id: String) {
        assert!(self.entries.contains_key(&id));
        self.root = id;
    }

    pub fn add_entry<'a>(&'a mut self, id: &str, title: String) -> &'a mut Entry {
        let ix = self.relationships.add_node(id.into());
        self.entries.insert(id.into(), (Entry::named(title), ix));
        let entry = &mut self.entries.get_mut(id).expect("fatal error: Entry ID not found").0;
        entry.graph_id = Some(ix);
        entry
    }

    pub fn get_entry_mut<'a>(&'a mut self, id: &str) -> Option<&'a mut Entry> {
        self.entries.get_mut(id).map(|(e,_)| e)
    }

     pub fn get_entry<'a>(&'a self, id: &str) -> Option<&'a Entry> {
        self.entries.get(id).map(|(e,_)| e)
    }

    pub fn add_relationship(&mut self, from: &Entry, to: &Entry, kind: Relationship) {
        self.relationships.add_edge(
            from.graph_id.expect("fatal error: must not add relationship from entry not connected to graph"), 
            to.graph_id.expect("fatal error: must not add relationship to entry not connected to graph"), 
            kind
        );
    }

    pub fn add_membership(&mut self, member: &Entry, entry: &Entry) {
        let member_id = member.graph_id.expect("fatal error: must not add relationship from entry not connected to graph");
        let entry_id = entry.graph_id.expect("fatal error: must not add relationship to entry not connected to graph");
        let has_owner = self.relationships
            .edges_directed(member_id, Incoming)
            .find(|r| *r.weight() == Relationship::Owner).is_some();

        if !has_owner {
            self.relationships.add_edge(member_id, entry_id, Relationship::Owner);
        }

        self.relationships.add_edge(entry_id, member_id, Relationship::Member);
    }

}

#[derive(Clone)]
pub enum EntryGraphError {

}

const DEFAULT_ENTRY_CAPACITY: usize = 1000;