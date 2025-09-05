use std::{
    num::ParseIntError, path::PathBuf
};
use strum_macros::AsRefStr;
use crate::{ 
    backend, 
    ir::{
        self, entry_graph::{AttributeLookupError, EntryLookupError, RoleLookupError}, rich_text::RichTextRepresentation, Entry, EntryGraph, Language, MemberSection, RichText
    }
};

use super::arguments::{
    Parameter,
    Arguments,
    ArgumentValue,
    ArgumentValueError,
};

#[derive(enum_assoc::Assoc, Clone, Debug)]
#[func(pub const fn id(&self) -> &'static str)]
pub enum Action<Builtin> {

    #[assoc(id = "builtin")]
    Builtin(Builtin),

    /// Creates a new entry and opens new scope for this entry
    #[assoc(id = "graph.entry.create")]
    CreateEntry {
        /// Entry identifier
        id: Parameter<String>,

        // Title of entry
        title: Parameter<String>,

        content: Parameter<Option<RichText>>,

        continue_content: bool 
    },

    /// Parses, generates sema, and attaches symbol to current entry.
    /// 
    /// To create a new entry with this symbol, invoke [``Action::CreateEntry``]
    /// before. And set its role using [``Action::SetRole``] to [``ir::Role``]
    #[assoc(id = "entry.symbol.add")]
    AddSymbol {
        raw: Parameter<String>,
        language: Parameter<Language>
    },

    /// Creates a member section with members of the scoped opened next
    #[assoc(id = "entry.member-section.create")]
    CreateMemberSection {
        title: Parameter<String>,

        content: Parameter<Option<RichText>>,

        continue_content: bool,
    },

    /// Moves content of the scope opened next to entry with specified ID
    #[assoc(id = "move-to-entry")]
    MoveToEntry {
        entry_id: Parameter<String>,

        content: Parameter<Option<RichText>>,

        continue_content: bool,
    },

    /// Indicates membership of current entry
    /// 
    /// If membership is added multiple times, the first membership constitues ownership.
    #[assoc(id = "entry.membership.add")]
    AddMembership {
        entry_id: Parameter<String>,
    },

    /// Sets abstract text of current entry
    #[assoc(id = "entry.abstract.set")]
    SetAbstract {
        content: Parameter<ir::RichText>
    },

    /// Sets attribute on current entry
    #[assoc(id = "entry.attribute.add")]
    AddAttribute {
        key: Parameter<String>,
        value: Parameter<ir::Value>,
    },

    /// Sets role of current entry
    #[assoc(id = "entry.role.set")]
    SetRole {
        role: Parameter<String>
    },

    /// Sets this entry as the root entry
    #[assoc(id = "graph.root.set-this")]
    SetRootEntry,

    /// Assigns current symbol to other entry
    #[assoc(id = "symbol.assign-to-entry")]
    AssignSymbol {
        entry_id: Parameter<String>,
    },

    /// Sets symbol help text of current symbol
    #[assoc(id = "symbol.help.set")]
    SetSymbolHelp {
        content: Parameter<ir::RichText>
    },

    /// Sets symbol heading of current symbol
    #[assoc(id = "symbol.heading.set")]
    SetSymbolHeading {
        content: Parameter<String>
    },

    /// Adds section item, such as parameter or return value
    #[assoc(id = "entry.section.items.add")]
    AddSectionItem {
        kind: ir::CharacteristicSection,
        value: Parameter<ir::RichText>,
        description: Parameter<ir::RichText>,
        modifier: Parameter<Vec<ir::Modifier>>,
    },

    /// Sets abstract paragraph of section, such as return value or throws
    #[assoc(id = "entry.section.abstract.set")]
    SetSectionAbstract {
        kind: ir::CharacteristicSection,
        content: Parameter<ir::RichText>,
    },

    /// Inserts rich text
    #[assoc(id = "block.insert-admonition")]
    InsertAdmonition {
        contents: Parameter<ir::RichText>
    },

    #[assoc(id = "inline.insert-text")]
    InsertText {
        text: Parameter<String>,
    },

    #[assoc(id = "inline.insert")]
    Insert {
        content: Parameter<Option<ir::RichText>>,
        continue_content: bool,
    },

    /// Insert contents of scope opened next into specified output
    #[assoc(id = "inline.insert-with-output-restriction")]
    InsertOnlyIntoOutput {
        format: backend::Format,
        content: Parameter<Option<ir::RichText>>,
        continue_content: bool,
    },

    /// Insert into specified output from file
    #[assoc(id = "inline.insert-from-file")]
    InsertIntoOutputFromFile {
        format: backend::Format,
        file: Parameter<PathBuf>
    },

    #[assoc(id = "inline.insert-reference")]
    /// Inserts reference to another entry with specified id
    InsertReference {
        entry_id: Parameter<String>
    },
}

impl<'a, Builtin> Into<&'static str> for &'a Action<Builtin> where &'a Builtin: Into<&'static str> {
    fn into(self) -> &'static str {
        match self {
            Action::Builtin(builtin) => builtin.into(),
            action => action.id()
        }
    }
}

impl<'a, Builtin> Action<Builtin> where &'a Builtin: Into<&'static str>, Self: 'a {
    pub fn action_identifier(&'a self) -> &'static str {
        self.into()
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum ActionError {
    #[error(transparent)]
    ArgumentValueFailure(#[from] ArgumentValueError<ParseIntError>),

    #[error(transparent)]
    ArgumentValueFailure2(#[from] ArgumentValueError),

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

    fn graph(&mut self) -> Result<&'a mut EntryGraph, ActionError>;

    fn execute_builtin(&mut self, action: Self::BuiltinAction) -> Result<(), ActionError>;

    fn enter_entry_scope(&mut self, entry: &mut Entry, id: &str) -> Result<(), ActionError>;
    fn leave_entry_scope(&mut self, entry: &mut Entry, id: &str) -> Result<(), ActionError>;

    fn current_entry(&mut self, purpose: &'static str) -> Result<(&'a mut Entry, String), ActionError>;

    fn current_member_section(&mut self) -> Result<&'a MemberSection, ActionError>;

    fn current_content(&mut self) -> Result<&mut RichText, ActionError>;

    fn process_content(&mut self, content: RichText) -> Result<RichText, ActionError>;

    fn preferred_rich_text_representation(&self) -> Option<RichTextRepresentation>;

    fn insert_content(&mut self, content: RichText) -> Result<(), ActionError> {
        let content = self.process_content(content)?;
        self.current_content()?.append(content);
        Ok(())
    }

    fn execute(&mut self, action: Action<Self::BuiltinAction>, arguments: Arguments) -> Result<(), ActionError> {
        match action {
            Action::Builtin(builtin) => 
                self.execute_builtin(builtin),
            Action::CreateEntry { 
                id, 
                title, 
                content,
                continue_content
            } => {
                let mut arguments = arguments;
                let content = arguments.resolve_rich_text_optional(
                    content, 
                    self.preferred_rich_text_representation()
                );
                let id = arguments.resolve_str(&id)?;
                let title = arguments.resolve_str(&title)?;
                let entry = self.graph()?.add_entry(&id, title.into());
                
                if content.is_some() || continue_content {
                    self.enter_entry_scope(entry, &id)?;
                }

                let res =
                if let Some(content) = content {
                    self.insert_content(content)
                } else {
                    Ok(())
                };

                if !continue_content {
                    self.leave_entry_scope(entry, &id)?;
                }

                res?;
                Ok(())
            }
            Action::MoveToEntry { 
                entry_id,
                content,
                continue_content
            } => {
                let mut arguments = arguments;
                let content = arguments.resolve_rich_text_optional(
                    content, 
                    self.preferred_rich_text_representation()
                );
                let id = arguments.resolve_str(&entry_id)?;

                let entry = self.graph()?
                    .get_entry(&id)
                    .ok_or_else(|| EntryLookupError::UnknownEntryID(id.clone().into_string()))?;

                if content.is_some() || continue_content {
                    self.enter_entry_scope(entry, &id)?;
                }

                let res =
                if let Some(content) = content {
                    self.insert_content(content)
                } else {
                    Ok(())
                };

                if !continue_content {
                    self.leave_entry_scope(entry, &id)?;
                }

                res?;
                Ok(())
            }
            Action::CreateMemberSection { 
                title, 
                content, 
                continue_content 
            } => {
                let mut arguments = arguments;
                let content = arguments.resolve_rich_text_optional(
                    content, 
                    self.preferred_rich_text_representation()
                );
                let title = arguments.resolve_str(&title)?;

                self.current_entry("create member section")?.0;

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
                let mut arguments = arguments;
                let key = arguments.resolve_str(&key)?.into_string();
                let attribute = self.graph()?.attributes.get(&key)
                    .ok_or_else(|| AttributeLookupError::UnknownAttributeKey(key.clone()))?;

                let value = arguments.resolve_converted(value, attribute.value_type)?;

                let entry = self.current_entry("add attribute")?.0;
                
                if attribute.repeatable {
                    entry.add_attribute(key, value);
                } else {
                    entry.set_attribute(key, value);
                }
                Ok(())
            }
            Action::AddMembership { entry_id } => {
                let graph = self.graph()?;
                let id = arguments.resolve_str(&entry_id)?;
                let entry = graph.get_entry(&id)
                    .ok_or_else(|| EntryLookupError::UnknownEntryID(id.clone().into()))?;

                if !entry.can_add_member(self.current_entry("get role")?.0.role) {
                    return Err(ActionError::LeafEntry(entry.title.clone()))
                }
                entry.add_member(id.into_string());
                Ok(())
            }
            Action::SetAbstract { content } => {
                let mut arguments = arguments;
                let abstract_ = arguments.resolve_rich_text(content, None)?;
                self.current_entry("set abstract")?.0.set_abstract(abstract_);
                Ok(())
            }
            _ => {
                Err(ActionError::UnsupportedAction(action.id()))
            }
        }
    }
}