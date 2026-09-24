use std::{
    borrow::Cow, num::ParseIntError, path::PathBuf
};
use enum_assoc::Assoc;
use serde::{Deserialize, Serialize};
use strum_macros::AsRefStr;
use crate::{ 
    backend, compiler::{diagnostics::{self, DiagnosticReporter}, strings::{self, CowStr}}, frontend, ir::{
        self, Entry, EntryGraph, Language, MemberSection, RichText, entry_graph::{AttributeLookupError, EntryLookupError, RoleLookupError}, rich_text::{self, RichTextRepresentation}
    }
};

use super::arguments::{
    Parameter,
    Arguments,
    ArgumentValue,
    ArgumentValueError,
};

use serde_variant::to_variant_name;

const fn _true() -> bool { true }

pub type Script<'a, Builtin> = Vec<Action<'a, Builtin>>;

#[derive(Clone, Debug, Assoc)]
#[func(pub const fn id(&self) -> &'static str)]
pub enum Action<'a, Builtin> {

    #[assoc(id = "builtin")]
    Builtin(Builtin),

    #[assoc(id = "script")]
    Script(CowStr<'a>),

    #[assoc(id = "parameter.set")]
    SetParameter {
        argument: Parameter<CowStr<'a>>,
        value: Parameter<ir::Value<'a>>,
    },

    /// Creates a new entry and opens new scope for this entry
    #[assoc(id = "graph.entry+")]
    CreateEntry {
        /// Entry identifier
        id: Parameter<CowStr<'a>>,

        // Title of entry
        title: Parameter<CowStr<'a>>,

        content: Parameter<Option<RichText>>,
    },

    /// Parses, generates sema, and attaches symbol to current entry.
    /// 
    /// To create a new entry with this symbol, invoke [``Action::CreateEntry``]
    /// before. And set its role using [``Action::SetRole``] to [``ir::Role``]
    #[assoc(id = "entry.symbol+")]
    AddSymbol {
        raw: Parameter<CowStr<'a>>,
        language: Parameter<Language>
    },

    /// Creates a member section with members of the scoped opened next
    #[assoc(id = "entry.member-section+")]
    CreateMemberSection {
        title: Parameter<CowStr<'a>>,

        content: Parameter<Option<RichText>>,
    },

    #[assoc(id = "scope.push")]
    PushScope {
        name: Parameter<Option<CowStr<'a>>>,
    },

    #[assoc(id = "scope.pop")]
    PopScope {
        name: Parameter<Option<CowStr<'a>>>,
    },

    #[assoc(id = "scope.boundary.apply")]
    SetScopeBoundary {
        beyond_comment: Parameter<bool>
    },


    #[assoc(id = "scope.entry.set")]
    SetEntry {
        entry_id: Parameter<CowStr<'a>>,
    },


    #[assoc(id = "scope.attribute.apply")]
    ApplyAttribute {
        key: Parameter<CowStr<'a>>,
        value: Parameter<ir::Value<'a>>,
    },

    #[assoc(id = "scope.output.allow")]
    AddAllowedOutputBackend {
        allowed: Parameter<backend::Format>
    },

    #[assoc(id = "scope.output.disallow")]
    AddDisallowedOutputBackend {
        disallowed: Parameter<backend::Format>
    },

    /// Insert contents of scope opened next into specified output
    /// 
    /// Treats everything that follows as verbatim output until a command
    /// with a matching [`Self::PopScope`] action is found. Syntax in between
    /// that is a valid command is ignored unless it is the one containing
    /// [`Self::PopScope`] or one that has [`Self::Include`] with a matching
    /// language that can be included in the output.
    #[assoc(id = "scope.language.set")]
    SetScopeLanguage {
        language: Parameter<ir::Language>,
    },

    #[assoc(id = "diagnose")]
    Diagnose {
        level: Parameter<ir::DiagnosticLevel>,
        message: Parameter<CowStr<'a>>,
    },

    #[assoc(id = "entry.appearance.option.add")]
    SetDisplayOption {
        name: Parameter<CowStr<'a>>,
        value: Parameter<ir::Value<'a>>
    },

    /// Indicates membership of current entry
    /// 
    /// If membership is added multiple times, the first membership constitues ownership.
    #[assoc(id = "entry.membership.add")]
    AddMembership {
        entry_id: Parameter<CowStr<'a>>,
    },

    /// Indicates membership of current entry
    /// 
    /// If membership is added multiple times, the first membership constitues ownership.
    #[assoc(id = "entry.relationship.add")]
    AddRelationship {
        kind: Parameter<ir::Relationship>,
        entry_id: Parameter<CowStr<'a>>,
    },

    /// Sets abstract text of current entry
    #[assoc(id = "entry.abstract.set")]
    SetAbstract {
        content: Parameter<ir::RichText>
    },

    /// Sets attribute on current entry
    #[assoc(id = "entry.attribute.add")]
    AddAttribute {
        key: Parameter<CowStr<'a>>,
        value: Parameter<ir::Value<'a>>,
    },

    /// Sets role of current entry
    #[assoc(id = "entry.role.set")]
    SetRole {
        role: Parameter<CowStr<'a>>
    },

    /// Sets this entry as the root entry
    #[assoc(id = "graph.root.set")]
    SetRootEntry,

    /// Assigns current symbol to other entry
    #[assoc(id = "entry.symbol.add")]
    AssignSymbol {
        entry_id: Parameter<CowStr<'a>>,
    },

    /// Sets symbol help text of current symbol
    #[assoc(id = "symbol.help.set")]
    SetSymbolHelp {
        content: Parameter<ir::RichText>
    },

    /// Sets symbol heading of current symbol
    #[assoc(id = "symbol.heading.set")]
    SetSymbolHeading {
        content: Parameter<CowStr<'a>>
    },

    /// Adds section item, such as parameter or return value
    #[assoc(id = "entry.section.items.add")]
    AddSectionItem {
        kind: Parameter<ir::CharacteristicSection>,
        value: Parameter<ir::RichText>,
        description: Parameter<ir::RichText>,
    },

    /// Sets abstract paragraph of section, such as return value or throws
    #[assoc(id = "entry.section.abstract.set")]
    SetSectionAbstract {
        kind: Parameter<ir::CharacteristicSection>,
        content: Parameter<ir::RichText>,
    },

    /// Inserts rich text
    #[assoc(id = "block.admonition")]
    InsertAdmonition {
        contents: Parameter<ir::RichText>,
        kind: Parameter<ir::AdmonitionKind>
    },

    #[assoc(id = "block.code")]
    InsertCodeBlock {
        contents: Parameter<CowStr<'a>>,
        language: Parameter<Option<ir::Language>>,
    },

    #[assoc(id = "block.code.snippet")]
    InsertSnippet {
        // TODO: Like DocC snippets, document that here...
        source: Parameter<PathBuf>,
        language: Parameter<Option<ir::Language>>,
        snippet: Parameter<Option<CowStr<'a>>>
    },

    #[assoc(id = "block.code.start")]
    StartCodeBlock {
        language: Parameter<Option<ir::Language>>,
    },

    #[assoc(id = "block.code.end")]
    EndCodeBlock {
        
    },


    #[assoc(id = "image")]
    InsertImage {
        // https://www.swift.org/documentation/docc/adding-images
        image_source: Parameter<PathBuf>,
        caption: Parameter<Option<ir::RichText>>,
        inline: Parameter<bool>,
        id: Parameter<Option<CowStr<'a>>>,
    },

    #[assoc(id = "block.video")]
    InsertVideo {
        // https://www.swift.org/documentation/docc/video
        video_source: Parameter<PathBuf>,
        poster_source: Parameter<PathBuf>,
        caption: Parameter<Option<ir::RichText>>,
        id: Parameter<Option<CowStr<'a>>>,
    },

    #[assoc(id = "block.audio")]
    InsertAudio {
        audio_source: Parameter<PathBuf>,
        caption: Parameter<Option<ir::RichText>>,
        id: Parameter<Option<CowStr<'a>>>,
    },

    #[assoc(id = "headline")]
    InsertHeadline {
        id: Parameter<Option<CowStr<'a>>>,
        level: Parameter<u8>,
        content: Parameter<ir::RichText>
    },

    #[assoc(id = "list.item")]
    InsertListItem {
        style: Parameter<rich_text::ListStyle>,
        content: Parameter<ir::RichText>
    },

    #[assoc(id = "inline.styled")]
    InsertStyled {
        text: Parameter<CowStr<'a>>,
        style: Parameter<ir::rich_text::InlineTextStyle>
    },

    #[assoc(id = "inline.text")]
    InsertText {
        text: Parameter<CowStr<'a>>,
    },

    #[assoc(id = "inline.emoji")]
    InsertEmoji {
        name: Parameter<CowStr<'a>>,
    },

    #[assoc(id = "inline.code")]
    InsertCode {
        content: Parameter<CowStr<'a>>,
        language: Parameter<Option<ir::Language>>,
    },


    #[assoc(id = "inline.reference")]
    /// Inserts reference to another entry with specified id
    InsertReference {
        entry_id: Parameter<CowStr<'a>>
    },

    #[assoc(id = "inline")]
    Insert {
        content: Parameter<Option<ir::RichText>>,
    },

    #[assoc(id = "embed.file")]
    EmbedFromFile {
        language: Parameter<Option<ir::Language>>,
        source: Parameter<PathBuf>,
    },

    #[assoc(id = "embed")]
    Embed {
        language: Parameter<Option<ir::Language>>,
        content: Parameter<CowStr<'a>>,
    },
}

impl<'a, Builtin> Into<CowStr<'a>> for &'a Action<'a, Builtin> where &'a Builtin: Into<CowStr<'a>>, Self: Serialize, Builtin: Serialize {

    fn into(self) -> CowStr<'a> {
        match self {
            Action::Builtin(builtin) => format!("builtin.{}", to_variant_name(builtin).unwrap()).into(),
            Action::Script(name) => format!("script.{}", name.as_ref()).into(),
            _ => to_variant_name(&self).unwrap().into()
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    #[error(transparent)]
    ArgumentValueFailure(#[from] ArgumentValueError<ParseIntError>),

    #[error(transparent)]
    ArgumentValueFailure2(#[from] ArgumentValueError),

    #[error(transparent)]
    ArgumentValueFailure3(#[from] ArgumentValueError<strum::ParseError>),

    #[error(transparent)]
    LanguageError(#[from] frontend::LanguageError),

    #[error(transparent)]
    RoleLookupFailure(#[from] RoleLookupError),

    #[error(transparent)]
    AttributeLookupFailure(#[from] AttributeLookupError),

    #[error(transparent)]
    EntryLookupFailure(#[from] EntryLookupError),

    #[error("Entry '{0}' does not support member entries")]
    LeafEntry(String),

    #[error("Action '{0}' is unsupported, please file a bug report")]
    UnsupportedAction(&'static str),
}

pub trait ActionExecutor<'a> {
    type BuiltinAction;
    type D : diagnostics::DiagnosticReporter;

    fn graph(&mut self) -> Result<&'a mut EntryGraph, ActionError>;

    fn execute_builtin(&mut self, action: &Self::BuiltinAction) -> Result<(), ActionError>;

    fn enter_entry_scope(&mut self, entry: &mut Entry, id: &str) -> Result<(), ActionError>;
    fn leave_entry_scope(&mut self, entry: &mut Entry, id: &str) -> Result<(), ActionError>;

    fn current_entry(&mut self, purpose: &'static str) -> Result<(&'a mut Entry, String), ActionError>;

    fn enter_member_section(&mut self, section: &mut MemberSection) -> Result<(), ActionError>;
    fn leave_member_section(&mut self, section: &mut MemberSection) -> Result<(), ActionError>;
    fn current_member_section(&mut self) -> Result<&'a MemberSection, ActionError>;

    fn current_content(&mut self) -> Result<&mut RichText, ActionError>;

    fn process_content(&mut self, content: RichText) -> Result<RichText, ActionError>;

    fn preferred_rich_text_representation(&self) -> Option<RichTextRepresentation>;

    fn insert_content(&mut self, content: RichText) -> Result<(), ActionError> {
        let content = self.process_content(content)?;
        self.current_content()?.append(content);
        Ok(())
    }

    fn default_diags(&self) -> &Self::D;

    fn language_config(&self) -> &frontend::LanguageSettings;

    fn execute<'b>(&mut self, action: &Action<'a, Self::BuiltinAction>, arguments: &'b mut Arguments<'b>) -> Result<(), ActionError> where 'a : 'b {
        match action {
            Action::Builtin(builtin) => 
                self.execute_builtin(builtin),
            Action::CreateEntry { 
                id, 
                title, 
                content,
            } => {
                let arguments = arguments;
                let content = arguments.resolve_rich_text_optional(
                    content, 
                    self.preferred_rich_text_representation()
                );
                let id = arguments.resolve_str(&id)?;
                let title = arguments.resolve_str(&title)?;
                let entry = self.graph()?.add_entry(&id, title.into());
                
                if content.is_some() {
                    self.enter_entry_scope(entry, &id)?;
                }

                let res =
                if let Some(content) = content {
                    self.insert_content(content)
                } else {
                    Ok(())
                };

                // if !continue_content {
                //     self.leave_entry_scope(entry, &id)?;
                // }

                res?;
                Ok(())
            }
            // Action::MergeInto { 
            //     entry_id,
            //     content,
            //     continue_content
            // } => {
            //     let arguments = arguments;
            //     let content = arguments.resolve_rich_text_optional(
            //         content, 
            //         self.preferred_rich_text_representation()
            //     );
            //     let id = arguments.resolve_str(&entry_id)?;

            //     let entry = self.graph()?
            //         .get_entry_mut(&id)
            //         .ok_or_else(|| EntryLookupError::UnknownEntryID(id.clone().into_string()))?;

            //     if content.is_some() || continue_content {
            //         self.enter_entry_scope(entry, &id)?;
            //     }

            //     let res =
            //     if let Some(content) = content {
            //         self.insert_content(content)
            //     } else {
            //         Ok(())
            //     };

            //     if !continue_content {
            //         self.leave_entry_scope(entry, &id)?;
            //     }

            //     res?;
            //     Ok(())
            // }
            Action::CreateMemberSection { 
                title, 
                content, 
            } => {
                let arguments = arguments;
                let content = arguments.resolve_rich_text_optional(
                    content, 
                    self.preferred_rich_text_representation()
                );
                let title = arguments.resolve_str(&title)?;

                let entry = self.current_entry("create member section")?.0;

                let section = entry.add_member_section(title.into_string(), None);
            
                if content.is_some() {
                    self.enter_member_section(section)?;
                }

                let res =
                if let Some(content) = content {
                    self.insert_content(content)
                } else {
                    Ok(())
                };

                res?;
                Ok(())
            }
            Action::AddSectionItem { 
                kind, 
                value: content, 
                description, 
            } => {
                let kind = arguments.resolve_parsed(kind)?;


                Ok(())
            }
            Action::SetRole { role } => {
                let role_key = arguments.resolve_str(&role)?;
                let role_id = self.graph()?.roles.id_for_key(&role_key)
                    .ok_or_else(|| RoleLookupError::UnknownRoleKey(role_key.into()))?;
                self.current_entry("set entry role")?.0.assign_role(role_id);
                Ok(())
            }
            Action::SetRootEntry => {
                self.graph()?.set_root(self.current_entry("set graph root")?.1.into());
                Ok(())
            }
            Action::AddAttribute { key, value } => {
                let arguments = arguments;
                let key = arguments.resolve_str(&key)?.into_string();
                let graph = self.graph()?;
                let attribute_id = graph.attributes.id_for_key(&key)
                    .ok_or_else(|| AttributeLookupError::UnknownAttributeKey(key.clone()))?;

                let attribute = graph.attributes.get_by_id(attribute_id)
                    .ok_or_else(|| AttributeLookupError::UnknownAttributeKey(key.clone()))?;

                let value = arguments.resolve_converted(value, attribute.value_type)?.into_static();

                let entry = self.current_entry("add attribute")?.0;
                
                if attribute.repeatable {
                    entry.attributes.add_attribute(attribute_id, value);
                } else {
                    entry.attributes.set_attribute(attribute_id, value);
                }
                Ok(())
            }
            Action::AddMembership { entry_id } => {
                let id = arguments.resolve_str(&entry_id)?;
                let entry = self.graph()?.get_entry(&id)
                    .ok_or_else(|| EntryLookupError::UnknownEntryID(id.clone().into()))?;

                let current_entry = self.current_entry("add membership")?.0;
                if !entry.can_add_member(current_entry.role) {
                    return Err(ActionError::LeafEntry(entry.title.clone()))
                }
                
                self.graph()?.add_membership(current_entry, entry);
                Ok(())
            }
            Action::AddRelationship { kind, entry_id } => {
                let id = arguments.resolve_str(&entry_id)?;
                let kind = arguments.resolve_parsed(kind)?;
                let entry = self.graph()?.get_entry(&id)
                    .ok_or_else(|| EntryLookupError::UnknownEntryID(id.clone().into()))?;

                let current_entry = self.current_entry("add relationship")?.0;
                
                self.graph()?.add_relationship(current_entry, entry, kind);
                Ok(())
            }
            Action::SetAbstract { content } => {
                let arguments = arguments;
                let abstract_ = arguments.resolve_rich_text(content, None)?;
                self.current_entry("set abstract")?.0.set_abstract(abstract_);
                _ = arguments;
                Ok(())
            }
            Action::InsertAdmonition { 
                contents,
                kind
            } => {
                let kind = arguments.resolve_parsed(kind)?;
                let contents = arguments.resolve_rich_text(contents, self.preferred_rich_text_representation())?;
                self.process_content(contents.into_admonition(kind))?;
                _ = arguments;

                Ok(())
            }
            Action::AddSymbol { 
                raw, 
                language 
            } => {
                let language = arguments.resolve_parsed(language)?;
                let raw = arguments.resolve_str(&raw)?;
                let symbols = frontend::collect_language_symbols(
                    language, 
                    strings::Encoding::UTF8, 
                    raw.as_bytes(), 
                    self.language_config(), 
                    self.default_diags()
                )?;

                self.current_entry("add symbol")?.0.add_symbols(symbols);
                    
                Ok(())
            }
            _ => {
                Err(ActionError::UnsupportedAction(action.id()))
            }
        }
    }
}