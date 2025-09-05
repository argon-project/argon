use std::collections::HashMap;
use pulldown_cmark::doc_tags;

use super::{
    DoxygenDelimitedParameters,
    DoxygenCommand,
    DoxygenArgumentValidator,
    actions::{
        Action,
        Argument,
    },
};

use crate::{
    frontend::actions,
    ir
};

const DOXYGEN_ALLOWED_ENCLOSING_DELIMITERS: doc_tags::EnclosingDelimiters = 
    doc_tags::EnclosingDelimiters::CURLY_BRACES
    .union(doc_tags::EnclosingDelimiters::SQUARE_BRACKETS);

pub(crate) static DOXYGEN_DEFAULT_SYNTAX: doc_tags::Syntax = doc_tags::Syntax {
    allow_inline: true,
    allow_nesting: true,
    argument_syntax: doc_tags::ArgumentSyntax::DelimitedSuffix(DOXYGEN_ALLOWED_ENCLOSING_DELIMITERS),
    interrupts_paragraph: false,
    attachment_syntax: doc_tags::AttachmentSyntax::none()
};

impl DoxygenCommand {
    fn _static(
        delimited_arguments: DoxygenDelimitedParameters<'static>,
        required_whitespace_arguments: &'static [&'static str],
        optional_whitespace_arguments: &'static [&'static str],
        // Attachment
        // 
        // None ... no attachment
        // Some((name, required)) ... attachment, required if required is true, optional otherwise
        attachment: Option<(&'static str, bool)>,
        attaches_until_interrupt: bool,
        actions: Vec<actions::Action>,
        validator: Option<DoxygenArgumentValidator>,
    ) -> Self {
        let total_whitespace_sep_args = required_whitespace_arguments.len() + optional_whitespace_arguments.len();
        assert!(total_whitespace_sep_args < u8::max_value().into());
        Self {
            delimited_parameters: delimited_arguments,
            required_whitespace_parameters: required_whitespace_arguments.iter().map(|s| s.to_string()).collect(),
            optional_whitespace_parameters: optional_whitespace_arguments.iter().map(|s| s.to_string()).collect(),
            actions: actions,
            validator,
            syntax: doc_tags::Syntax {
                allow_nesting: true,
                allow_inline: true,
                interrupts_paragraph: attaches_until_interrupt && attachment.is_some(),
                // We allow both {} and [] delimiters in the syntax to diagnose when the wrong
                // delimiters are used. If we don't pass both, the arguments won't be
                // recognized, preventing us from emitting a warning.
                argument_syntax: doc_tags::ArgumentSyntax::DelimitedSuffix(DOXYGEN_ALLOWED_ENCLOSING_DELIMITERS),
                attachment_syntax: if attachment.is_some() {
                    if attaches_until_interrupt {
                        doc_tags::AttachmentSyntax::UntilInterrupt(total_whitespace_sep_args as u8)
                    } else {
                        doc_tags::AttachmentSyntax::UntilNewline(total_whitespace_sep_args as u8)
                    }
                } else {
                    doc_tags::AttachmentSyntax::OnlyWhitespaceSeparated(total_whitespace_sep_args as u8)
                }
            }
        }
    }
}

pub fn builtins() -> HashMap<String, DoxygenCommand> {
    HashMap::from([
        (
            "param".into(), DoxygenCommand::_static(
                DoxygenDelimitedParameters::Keywords(&["in", "out", "inout"]), 
                &["name"], 
                &["'-'"], 
                Some(("description", true)), 
                true, 
                vec![
                    Action::AddSectionItem { 
                        kind: ir::CharacteristicSection::Parameters, 
                        value: Argument::_Variable("name"), 
                        description: Argument::_Variable("description"), 
                        modifier: Argument::Constant(vec![])
                    }
                ], 
                None
            )
        )
    ])
}