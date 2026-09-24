pub mod predefined;
pub mod directive;

use directive::*;

use std::{cmp::min, collections::HashMap, convert::Infallible, iter::zip, ops::Range, path::PathBuf, str::FromStr, sync::{Arc, Mutex}, vec};

use pulldown_cmark::{
    doc_tags, CowStr, Event, Tag, TagEnd
};
use thiserror;
use tree_sitter::Point;

use crate::{
    compiler::{diagnostics::{self, DiagnosticReporter, EditorLocation, LocatedDiagnostic, LocationAttachableDiagnostic}, strings::Location}, driver, frontend::{
        self, SymbolCommentIngestingFrontend, actions::{
            self, Action,
        }, arguments::{
            ArgumentValue, ArgumentValueError, Arguments, Parameter
        }, c::{self, sema_gen::CLanguageRecorder}, cmark_comment::{
            CMARK_OPTIONS, options_from_prefix
        }
    }, ir::{self, RoleID, entry_graph::{AttributeLookupError, RoleLookupError}, rich_text::CommonMark},
};

const DOXYGEN_CMARK_OPTIONS: pulldown_cmark::Options = CMARK_OPTIONS
    .union(pulldown_cmark::Options::ENABLE_AT_DOC_TAGS)
    .union(pulldown_cmark::Options::ENABLE_BACKSLASH_DOC_TAGS);

pub(crate) const DOXYGEN_ALLOWED_ENCLOSING_DELIMITERS: doc_tags::EnclosingDelimiters = 
    doc_tags::EnclosingDelimiters::CURLY_BRACES
    .union(doc_tags::EnclosingDelimiters::SQUARE_BRACKETS);

pub(crate) static DOXYGEN_DEFAULT_SYNTAX: doc_tags::Syntax = doc_tags::Syntax {
    allow_inline: true,
    allow_nesting: true,
    argument_syntax: doc_tags::ArgumentSyntax::DelimitedSuffix(DOXYGEN_ALLOWED_ENCLOSING_DELIMITERS),
    interrupts_paragraph: false,
    attachment_syntax: doc_tags::AttachmentSyntax::none()
};

#[derive(Debug, Clone, Default)]
pub struct DoxygenSettings {
    pub directives: HashMap<String, DoxygenDirective<'static>>,
}

#[derive(thiserror::Error, Debug)]
pub enum DoxygenError {
    #[error("Found unknown command '{0}'")]
    UnknownCommand(String),

    #[error("Value supplied to option argument '{1}' of command '{0}'")]
    UnexpectedOptionValue(String, String, String),

    #[error("Unknown argument '{1}' passed to command '{0}'")]
    UnknownArgument(String, String),

    #[error("Argument delimiters {1}..{2} supplied, but command '{0}' takes no arguments")]
    UnexpectedArgumentDelimiters(String, char, char),

    #[error("Arguments passed to command '{0}' that takes no arguments")]
    UnexpectedArguments(String),

    #[error("Command '{0}' expects arguments in {1}..{2}, but passed in {3}..{4}")]
    MismatchingArgumentDelimiters(String, char, char, char, char),

    #[error("Too few arguments supplied to command '{0}'. Expecting {1}, but {2} were given.")]
    MissingArguments(String, usize, usize),

    #[error("Too many arguments supplied to command '{0}'. Expecting at most {1}, but {2} were given.")]
    AdditionalArguments(String, usize, usize),

    #[error("Missing {n} arguments for {names} in '{0}' command", n = (.1).len(), names = (.1).iter().map(|n| "'".to_string() + n + "'").collect::<Vec<String>>().join(", "))]
    MissingArguments2(String, Vec<String>),

    #[error("Cannot modify '{0}' of current entry as there is none")]
    LackingContext(&'static str),

    #[error(transparent)]
    RoleFailure(#[from] RoleLookupError),

    #[error(transparent)]
    AttributeFailure(#[from] AttributeLookupError),

    #[error(transparent)]
    ArgumentValueFailure(#[from] ArgumentValueError),

    #[error("Doxygen frontend does not support {0} action")]
    UnsupportedAction(String),

    #[error("Doxygen directive '{0}' is not implemented")]
    UnimplementedDirective(String),
}

impl diagnostics::Diagnostic for DoxygenError {
    fn severity(&self) -> diagnostics::Severity {
        match self {
            _ => diagnostics::Severity::Error
        }
    }

    fn is_internal(&self) -> bool {
        match self {
            _ => false,
        }
    }
}

pub struct DoxygenFrontend<'a, D: DiagnosticReporter> {
    graph: Arc<Mutex<ir::EntryGraph>>,
    settings: &'a DoxygenSettings,
    path: PathBuf,
    language: Option<ir::Language>,
    config: &'a driver::Config,
    current_entry: Option<(&'a str, &'a mut ir::Entry)>,
    diags: &'a D
}

impl<'a, D: DiagnosticReporter> DoxygenFrontend<'a, D> {
    pub fn new(
        graph: Arc<Mutex<ir::EntryGraph>>, 
        settings: &'a DoxygenSettings, 
        path: PathBuf, 
        language: Option<ir::Language>, 
        config: &'a driver::Config, 
        diags: &'a D
    ) -> Self {
        Self {
            graph,
            settings,
            path,
            language,
            config,
            current_entry: None,
            diags
        }
    }
}

impl<'a, D: DiagnosticReporter> SymbolCommentIngestingFrontend for DoxygenFrontend<'a, D> {
    fn record_comment<'input>(
        &mut self,
        text: &'input str,
        prefix: &'static str,
        comment_location: &impl diagnostics::EditorLocation,
    ) {
        // We want to emit human-readable diagnostics. For that, we need line numbers.
        // Get the indices of the first character of each line.
        let line_resolver = diagnostics::LineResolver::new(text);

        let options = DOXYGEN_CMARK_OPTIONS
            .union(options_from_prefix(prefix));

        let commands = &self.settings.directives;

        let provide_doxygen_tag_syntax = |name: &str, _options: pulldown_cmark::Options| -> Option<&pulldown_cmark::doc_tags::Syntax> {
            let Some(command) = commands.get(name) else {
                // We still want to provide a syntax description even if the command is not known.
                // Otherwise, the command will not be parsed and will not be included in the output.
                // However, we want to warn about unknown commands.
                // Hence, we return a default syntax here.
                return Some(&DOXYGEN_DEFAULT_SYNTAX)
            };

            Some(&command.syntax)
        };

        let parser = pulldown_cmark::Parser::new_with_doc_tag_syntax_provider_callback(
            text,
            options, 
            provide_doxygen_tag_syntax
        );

        let mut events = parser.into_offset_iter();

        while let Some((event, range)) = events.next() {
            let Event::Start(Tag::DocTag { 
                name, 
                arguments: delimited_arguments, 
                argument_delimiters 
            }) = event else {
                continue
            };

            let location = line_resolver.resolve(range.clone()).relative_to(comment_location);

            let Some(command) = commands.get(name.as_ref()) else {
                self.diags.diagnose(
                    DoxygenError::UnknownCommand(name.into_string())
                        .at(location.clone())
                );
                continue;
            };

            let mut arguments = Arguments::<'input>::default();

            let expected_delimiters = command.parameters.delimited.delimiters();
            if argument_delimiters != expected_delimiters {
                if expected_delimiters.is_none() {

                    // Only diagnose unexpected argument delimiters if there are no arguments
                    // and just the delimiters:
                    // - @command[] --> DoxygenError::UnexpectedArgumentDelimiters
                    // - @command[a,b,c] --> DoxygenError::UnexpectedArguments
                    if delimited_arguments.is_empty() {
                        self.diags.diagnose(
                            DoxygenError::UnexpectedArgumentDelimiters(
                                name.into_string(), 
                                argument_delimiters.unwrap().0 as char, 
                                argument_delimiters.unwrap().1 as char
                            )
                                .at(location.clone())
                        );
                        continue;
                    }
                } else {
                    self.diags.diagnose(
                        DoxygenError::MismatchingArgumentDelimiters(
                            name.into_string(), 
                            expected_delimiters.unwrap().0 as char,
                            expected_delimiters.unwrap().1 as char,
                            argument_delimiters.unwrap().0 as char, 
                            argument_delimiters.unwrap().1 as char
                        )
                            .at(location.clone())
                    );
                    continue;
                }
            }

            
            match &command.parameters.delimited {
                DoxygenDelimitedParameters::None => {
                    if !delimited_arguments.is_empty() {
                        self.diags.diagnose(DoxygenError::UnexpectedArguments(name.clone().into_string())
                            .at(location.clone())
                        );
                        continue;
                    }
                }

                DoxygenDelimitedParameters::KeywordModifiers(keywords) => {
                    let mut valid = true;

                    for (_, value_type, value) in &delimited_arguments {
                        
                        if !keywords.contains(&value.as_ref()) {
                            valid = false;
                            self.diags.diagnose(DoxygenError::UnknownArgument(name.clone().into_string(), value.as_ref().to_string())
                                .at(location.clone())
                            );
                            continue;
                        }

                        arguments.add(value.as_ref(), ArgumentValue::Void, location.clone());
                    }
                }

                DoxygenDelimitedParameters::Options(value_separator, option_keys) => {
                    let mut valid = true;

                    for (_, _, value) in &delimited_arguments {
                        let components = value.as_ref().split_once(*value_separator);
                        let option_key = components.map(|c| c.0).unwrap_or(value.as_ref());
                        let option_value = components.map(|c| c.1);

                        let Some(accepts_value) = option_keys.get(&option_key) else {
                            valid = false;
                            self.diags.diagnose(DoxygenError::UnknownArgument(
                                name.clone().into_string(), 
                                option_key.to_string()
                            )
                                .at(location.clone())
                            );
                            continue;
                        };

                        if let Some(option_value) = option_value {
                            if !*accepts_value {
                                valid = false;
                                self.diags.diagnose(DoxygenError::UnexpectedOptionValue(
                                    name.clone().into_string(), 
                                    value.clone().into_string(), 
                                    option_value.to_string()
                                )
                                    .at(location.clone())
                                );
                            }
                            continue;
                        }

                        arguments.add(option_key, ArgumentValue::from_opt_str(option_value), location.clone());
                    }
                }

                DoxygenDelimitedParameters::PositionalArguments(names, mandatory_count, optional_count) => {
                    if delimited_arguments.len() < *mandatory_count {
                        self.diags.diagnose(DoxygenError::MissingArguments(
                            name.clone().into_string(), 
                            *mandatory_count, 
                            delimited_arguments.len()
                        )
                            .at(line_resolver.resolve(range.clone()).relative_to(comment_location))
                        );
                        continue;
                    }

                    if delimited_arguments.len() > (mandatory_count + optional_count) {
                        self.diags.diagnose(DoxygenError::AdditionalArguments(
                            name.clone().into_string(), 
                            mandatory_count + optional_count, 
                            delimited_arguments.len()
                        )
                            .at(line_resolver.resolve(range.clone()).relative_to(comment_location))
                        );
                        continue;
                    }

                    for (name, value) in zip(*names, delimited_arguments.into_iter().map(|(_,_,v)| v)) {
                        arguments.add(name, ArgumentValue::PlainText(value.into()), location.clone());
                    }
                }
            };

            let whitespace_arguments: Vec<(&'input str, Range<usize>)> = events.by_ref().map_while(|(event, range)| {
                let Event::Text(arg) = &event else {
                    return None
                };

                Some((unsafe {std::mem::transmute(arg.as_ref()) }, range))
            }).collect();

            let min_whitespace_args = command.parameters.whitespace_separated.required.len();
            let max_whitespace_args = min_whitespace_args + command.parameters.whitespace_separated.optional.len();

            if whitespace_arguments.len() < min_whitespace_args {
                self.diags.diagnose(DoxygenError::MissingArguments(
                    name.clone().into_string(), 
                    min_whitespace_args, 
                    whitespace_arguments.len()
                )
                    // TODO: Use range of arguments instead, maybe provide a 'hint range' for the command
                    .at(line_resolver.resolve(range).relative_to(comment_location))
                );
                continue;
            }

            // TODO: Figure out if we need this check. Because the maximum number of whitespace-separated
            // arguments is set by the syntax, no more than that number should be recognized.
            // I think this is dead code.
            // Perhaps we should fail spectacularly here to indicate an error in pulldown-cmark.
            if whitespace_arguments.len() > max_whitespace_args {
                self.diags.diagnose(DoxygenError::AdditionalArguments(
                    name.clone().into_string(), 
                    max_whitespace_args, 
                    whitespace_arguments.len()
                )
                    // TODO: Use range of arguments instead, maybe provide a 'hint range' for the command
                    .at(location.clone())
                );
                continue;
            }

            let attachment: Vec<Event<'static>> = events.by_ref().map_while(|(e, _)| 
                if matches!(e, Event::End(TagEnd::DocTag)) {
                    Some(e.into_static())
                } else {
                    None
                }
            ).collect();

            if let Some(attached_parameter) = &command.parameters.attached {
                if attached_parameter.required && attachment.is_empty() {
                    self.diags.diagnose(DoxygenError::MissingArguments2(
                        name.clone().into_string(), 
                        vec![attached_parameter.name.to_string()]
                    )
                    // TODO: Use range of arguments instead, maybe provide a 'hint range' for the command
                    .at(location.clone())
                );

                arguments.add(
                    attached_parameter.name, 
                    ArgumentValue::RichText(ir::RichText::common_mark(
                        CommonMark::new(attachment)
                    )),
                    location
                );
            }
            } else {

                // TODO: Figure out if we need this check. Because the syntax specifies whether an attachment should be
                // recognized, a paragraph or text succeeding the last whitespace-separated argument should not be recognized
                // as argument to this command.
                // I think this is dead code.
                // Perhaps we should fail spectacularly here to indicate an error in pulldown-cmark.
                if !attachment.is_empty() {
                    self.diags.diagnose(DoxygenError::AdditionalArguments(
                        name.clone().into_string(), 
                        max_whitespace_args, 
                        whitespace_arguments.len() + 1
                    )
                        // TODO: Use range of arguments instead, maybe provide a 'hint range' for the command
                        .at(line_resolver.resolve(range.clone()).relative_to(comment_location))
                    );
                }
            }
            

            // We don't need to check if there's an attachment but none was expected or vice versa,
            // because the parser already checks that based on the syntax.

            // We should see the end of the doc tag now.
            let Some((Event::End(TagEnd::DocTag), _)) = events.next() else {
                continue;
            };

            self.evaluate_command(name.as_ref(), range.clone(), &line_resolver, &command, arguments, comment_location);
        }


    }
}

impl<'a, D: DiagnosticReporter> DoxygenFrontend<'a, D> {
    fn evaluate_command<'input>(
        &mut self, 
        name: &'input str,
        range: Range<usize>,
        line_resolver: &diagnostics::LineResolver,
        command: &DoxygenDirective,
        arguments: Arguments<'input>,
        comment_location: &impl diagnostics::EditorLocation,
    ) -> Result<(), DoxygenError> {
        if command.script.is_empty() {
            self.diags.diagnose(DoxygenError::UnimplementedDirective(
                name.into()
            )
                // TODO: Use range of arguments instead, maybe provide a 'hint range' for the command
                .at(line_resolver.resolve(range.clone()).relative_to(comment_location))
            );
        }

        Ok(())
    }

    fn evaluate_builtin<'input>(
        &mut self, 
        range: Range<usize>,
        line_resolver: &diagnostics::LineResolver,
        command: &DoxygenDirective,
        arguments: Arguments<'input>,
        builtin: DoxygenBuiltin
    ) -> Result<(), DoxygenError> {
        match builtin {
            DoxygenBuiltin::FileInfo => {
                for arg in arguments.iter() {
                    match arg.name {
                        "name" => {
                            let basename = self.path.file_name().and_then(|p|p.to_str());
                        }
                        "extension" => {
                            let extension = self.path.extension().and_then(|e| e.to_str());
                        }
                        "filename" => {
                            let filename = self.path.file_name().and_then(|e| e.to_str());
                        }
                        "directory" => {
                            let directory = self.path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str());
                        }
                        "full" => {
                            let path = self.path.to_str();
                        }
                        _ => {} // This is fine, the implemention has checked for unknown arguments earlier
                    }
                }

                if arguments.is_empty() {
                    let path: Option<&str> = self.path.to_str();
                }
            }

            DoxygenBuiltin::LineInfo => {
                let line_number = line_resolver.resolve(range).start_point.row + 1;
            }

            DoxygenBuiltin::File => {
                let path =
                if let Some(path) = arguments.get("path?") {
                    &PathBuf::from_str(&path.value.str()).unwrap()
                } else {
                    &self.path
                };

                let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
                    return Ok(())
                };

                let role = RoleID::from_extension(extension);
            }

            DoxygenBuiltin::Doxyconfig => {
                let config_option = arguments.get("option");
            }
        }

        Ok(())
    }

    fn perform_action<'input>(
        &mut self,
        range: Range<usize>,
        line_resolver: &diagnostics::LineResolver,
        command: &DoxygenDirective,
        arguments: Arguments<'input>,
        action: Action<DoxygenBuiltin>
    ) -> Result<(), DoxygenError> {
        Ok(())
    }

    fn insert_text(&mut self, text: &str) {
        
    }
}