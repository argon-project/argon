use std::{collections::HashMap, vec};
use pulldown_cmark::doc_tags;

use super::{
    DoxygenDelimitedParameters,
    DoxygenDirective,
    DoxygenArgumentValidator,
    actions::{
        Action,
    },
    frontend::arguments::*,
};

use crate::{
    backend, frontend::{actions::{self, Script}, doxygen::{DOXYGEN_ALLOWED_ENCLOSING_DELIMITERS, directive::{DoxygenAttachedParameter, DoxygenBuiltin, DoxygenParameters, DoxygenWhitespaceSeparatedParameters}}}, ir
};

impl<'a> DoxygenDirective<'a> {
    pub(crate) fn predefined(
        delimited_arguments: DoxygenDelimitedParameters<'static>,
        required_whitespace_arguments: &'static [&'static str],
        optional_whitespace_arguments: &'static [&'static str],
        // Attachment
        // 
        // None ... no attachment
        // Some((name, required)) ... attachment, required if required is true, optional otherwise
        attachment: Option<DoxygenAttachedParameter<'static>>,
        attaches_until_interrupt: bool,
        script: crate::frontend::actions::Script<'a, DoxygenBuiltin>,
        validator: Option<DoxygenArgumentValidator>,
    ) -> Self {
        let total_whitespace_sep_args = required_whitespace_arguments.len() + optional_whitespace_arguments.len();
        assert!(total_whitespace_sep_args < u8::max_value().into());
        Self {
            parameters: DoxygenParameters {
                delimited: delimited_arguments,
                whitespace_separated: DoxygenWhitespaceSeparatedParameters {
                    required: required_whitespace_arguments,
                    optional: optional_whitespace_arguments,
                },
                attached: attachment.clone(),
                validator,
            },
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
            },
            script
        }
    }
}

pub fn builtins() -> HashMap<String, DoxygenDirective<'static>> {
    HashMap::from([
        (
            "param".into(), DoxygenDirective::predefined(
                DoxygenDelimitedParameters::KeywordModifiers(&["in", "out", "inout"]), 
                &["name"], 
                &["'-'"], 
                Some(DoxygenAttachedParameter { name: "description", required: true }), 
                true, 
                vec![
                    
                ], 
                None
            )
        )
    ])
}

macro_rules! predefined_directives {
    (
        $(
            $name:literal => {
                $( delimited: $delim_variant:ident $delim_args:tt $(,)? )?
                $( required: [ $($required:expr),* ], )?
                $( optional: [ $($optional:expr),* ], )?
                $( attached: $att_name:literal { required: $att_req:expr, until: $att_until:ident $(,)? } $(,)? )?                
                $( validator: $validator:expr, )?
                $( script: $script:expr, )?
            }
        ),* $(,)?
    ) => {
        std::collections::HashMap::from([
            $(
                (
                    $name,
                    DoxygenDirective::predefined(
                        predefined_directives!(@_delimited $($delim_variant $delim_args)?),
                        &[ $($( $required ),*)? ],
                        &[ $($( $optional ),*)? ],
                        predefined_directives!(@_attached $($att_name, $att_req)?),
                        predefined_directives!(@until $($att_until)?),
                        predefined_directives!(@script $($script)?),
                        predefined_directives!(@validator $($validator)?),
                    )
                )
            ),*
        ])
    };

    (@_delimited) => { DoxygenDelimitedParameters::None };
    (@_delimited $variant:ident $args:tt) => { DoxygenDelimitedParameters::$variant $args };

    (@_attached) => { None };
    (@_attached $name:expr, $req:expr) => {
        Some(DoxygenAttachedParameter { name: $name, required: $req })
    };

    (@until) => { false };
    (@until newline) => { false };
    (@until interrupt) => { true };

    (@validator) => { None };
    (@validator $expr:expr) => { $expr };

    (@script) => { vec![] };
    (@script $expr:expr) => { $expr };
}

pub fn _predefined() -> std::collections::HashMap<&'static str, DoxygenDirective<'static>> {
    predefined_directives! {

        // MARK: Directives involving modifying or creating entries

        "addtogroup" => { 
            required: ["id"], 
            attached: "title" {
                required: false,
                until: newline
            },
            script: vec![
                Action::CreateEntry { 
                    id: param!("id"), 
                    title: param!("title"),
                    content: v!(None),
                },
                Action::PushScope {
                    name: param!("id")
                },
                Action::SetEntry {
                    entry_id: param!("id") 
                },
            ],
        },
        "weakgroup" => { 
            required: ["name"], 
            optional: ["title"], 
            script: vec![
                Action::CreateEntry { 
                    id: param!("id"), 
                    title: param!("title"),
                    content: v!(None),
                },
                Action::PushScope {
                    name: param!("id")
                },
                Action::SetEntry {
                    entry_id: param!("id") 
                },
            ],
        },
        "defgroup" => { 
            required: ["name"], 
            attached: "title" {
                required: true,
                until: newline
            },
            script: vec![
                Action::CreateEntry { 
                    id: param!("id"), 
                    title: param!("title"),
                    content: v!(None),
                },
                Action::PushScope {
                    name: param!("id")
                },
                Action::SetEntry {
                    entry_id: param!("id") 
                },
            ],
        },
        "page" => { 
            required: ["id"], 
            optional: ["title"], 
            script: vec![
                Action::CreateEntry { 
                    id: param!("id"), 
                    title: param!("title"),
                    content: v!(None),
                },
                Action::PushScope {
                    name: param!("id")
                },
                Action::SetEntry {
                    entry_id: param!("id") 
                },
                Action::SetRole {
                    role: s!("article")
                },
            ],
        },
        "mainpage" => { 
            optional: ["title"], 
            script: vec![
                Action::CreateEntry { 
                    id: param!("id"), 
                    title: param!("title"),
                    content: v!(None),
                },
                Action::PushScope {
                    name: param!("id")
                },
                Action::SetEntry {
                    entry_id: param!("id") 
                },
                Action::SetRole {
                    role: s!("article")
                },
                Action::SetRootEntry,
            ],
        },
        "subpage" => { 
            required: ["id"], 
            script: vec![
                Action::AddMembership { 
                    entry_id: param!("id")
                },
                Action::InsertReference {
                    entry_id: param!("id")
                }
            ],
        },
        "brief" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::SetAbstract {
                    content: param!("description")
                }
            ],
        },

        // MARK: - Relationships

        "ingroup" => { 
            required: ["id"],
            script: vec![
                Action::AddMembership { 
                    entry_id: param!("id")
                },
            ],
        },

        "memberof" => { 
            required: ["id"],
            script: vec![
                Action::AddMembership { 
                    entry_id: param!("id")
                },
            ],
        },

        "extends" => { 
            required: ["id"], 
            script: vec![
                Action::AddRelationship { 
                    kind: v!(ir::Relationship::predefined("api.extends")), 
                    entry_id: param!("id")
                }
            ],
        },

        "implements" => { 
            required: ["id"], 
            script: vec![
                Action::AddRelationship { 
                    kind: v!(ir::Relationship::predefined("api.implements")), 
                    entry_id: param!("id")
                }
            ],
        },

        "relates" => { 
            required: ["id"], 
             script: vec![
                Action::AddRelationship { 
                    kind: v!(ir::Relationship::predefined("")), 
                    entry_id: param!("id")
                }
            ],
        },
        "related" => { 
            required: ["id"], 
             script: vec![
                Action::AddRelationship { 
                    kind: v!(ir::Relationship::Related), 
                    entry_id: param!("id")
                }
            ],
        },
        "relatesalso" => { 
            required: ["id"], 
             script: vec![
                Action::AddRelationship { 
                    kind: v!(ir::Relationship::Related), 
                    entry_id: param!("id")
                }
            ],
        },
        "relatedalso" => { 
            required: ["identifier_node"], 
             script: vec![
                Action::AddRelationship { 
                    kind: v!(ir::Relationship::Related), 
                    entry_id: param!("id")
                }
            ],
        },

        // MARK: - Attributes 

        "protected" => {
            script: vec![
                Action::AddAttribute {
                    key: s!("api.visibility"), 
                    value: s!("protected") 
                }
            ],
        },

        "public" => {
            script: vec![
                Action::AddAttribute {
                    key: s!("api.visibility"), 
                    value: s!("public") 
                }
            ],
        },

        "private" => {
            script: vec![
                Action::AddAttribute {
                    key: s!("api.visibility"), 
                    value: s!("private") 
                }
            ],
        },

        "qualifier" => { 
            required: ["label"],
            script: vec![
                Action::AddAttribute { 
                    key: param!("label"), 
                    value: v!(ir::Value::Void) 
                }
            ],
        },

        "static" => { 
            script: vec![
                Action::AddAttribute { 
                    key: param!("api.instance-member"), 
                    value: v!(ir::Value::Boolean(false)) 
                }
            ],
        },

        "deprecated" => { 
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::AddAttribute { 
                    key: param!("api.availability.deprecated"), 
                    value: v!(ir::Value::Boolean(true)) 
                }
            ],
        },


        // MARK: - Scope

        "name" => { 
            optional: ["title"], 
            script: vec![
                Action::PushScope { 
                    name: s!("doxy.name")
                },
                Action::CreateMemberSection { 
                    title: param!("title"), 
                    content: v!(None) 
                }
            ],
        },

        "{" => {
            script: vec![
                Action::SetScopeBoundary { beyond_comment: v!(true) }
            ],
        },

        "}" => {
            script: vec![
                Action::PopScope { 
                    name: v!(None) 
                },
            ],
        },

        "internal" => {
            script: vec![
                Action::PushScope { 
                    name: s!("doxy.internal") 
                },
                Action::ApplyAttribute {
                    key: s!("api.visbility"), 
                    value: s!("internal") 
                }
            ],
        },
        "endinternal" => {
            script: vec![
                Action::PopScope { 
                    name: s!("doxy.internal")
                },
            ],
        },

        "publicsection" => {
             script: vec![
                Action::PushScope { 
                    name: s!("doxy.publicsection")
                },
                Action::SetScopeBoundary {
                    beyond_comment: v!(true)
                },
                Action::ApplyAttribute {
                    key: s!("api.visbility"), 
                    value: s!("public") 
                },
            ],
        },
        "protectedsection" => {
            script: vec![
                Action::PushScope { 
                    name: s!("doxy.protectedsection")
                },
                Action::SetScopeBoundary {
                    beyond_comment: v!(true)
                },
                Action::ApplyAttribute {
                    key: s!("api.visbility"), 
                    value: s!("protected") 
                },
            ],
        },
        "privatesection" => {
            script: vec![
                Action::PushScope { 
                    name: s!("doxy.privatesection")
                },
                Action::SetScopeBoundary {
                    beyond_comment: v!(true)
                },
                Action::ApplyAttribute {
                    key: s!("api.visbility"), 
                    value: s!("private") 
                },
            ],
        },


        
        "showrefby" => {
            script: vec![
               
            ],
        },
        "hiderefby" => {
            script: vec![
                
            ],
        },
        "showrefs" => {
            script: vec![
                
            ],
        },
        "hiderefs" => {
            script: vec![
                
            ],
        },
        "showinlinesource" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Capturing function source not supported")
                },
            ],
        },
        "hideinlinesource" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Capturing function source not supported")
                },
            ],
        },
        
        "showenumvalues" => {
            script: vec![
                Action::SetDisplayOption { 
                    name: s!("signature.enum.value"), 
                    value: v!(ir::Value::Boolean(true)) 
                }
            ],
        },
        "hideenumvalues" => {
            script: vec![
                Action::SetDisplayOption { 
                    name: s!("signature.enum.value"), 
                    value: v!(ir::Value::Boolean(false)) 
                }
            ],
        },

        // MARK: - Entry from artificial symbol

        "category" => { 
            required: ["name"], optional: ["file", "header"],
            script: vec![
                // TODO: figure out
            ],
        },
        "class" => { 
            required: ["name"], optional: ["file", "header"],
            script: vec![
                // TODO: figure out
            ],
        },
        "concept" => { 
            required: ["name"], 
        },
        "def" => { 
            required: ["name"], 
            script: vec![
                // TODO: figure out
            ],
        },
        "dir" => { 
            optional: ["path"],
            script: vec![
                // TODO: figure out
            ],
        },
        "enum" => { 
            required: ["name"], 
            script: vec![
                // TODO: figure out
            ],
        },
        "example" => { 
            delimited: Options('\0', std::collections::HashMap::from(
                [("lineno", false)
            ])), 
            required: ["filename"], 
            script: vec![
                // TODO: figure out
            ],
        },

        "fn" => { 
            attached: "declaration" {
                required: true, 
                until: newline, 
            },
        },
        "headerfile" => { 
            required: ["file"], 
            optional: ["name"],
        },
        "hideinitializer" => {},
        "idlexcept" => { 
            required: ["name"], 
        },
        
        "interface" => { 
            required: ["name"], 
            optional: ["file", "header"], 
        },
        
        "module" => { 
            required: ["name"], 
        },
        
        "namespace" => { 
            required: ["name"], 
        },
        "nosubgrouping" => {},
        "overload" => { 
            optional: ["declaration"], 
        },
        "package" => { 
            required: ["name"], 
        },
        
        
        
        "property" => { 
            attached: "declaration" {
                required: true, 
                until: newline, 
            },
        },
        
        
        "protocol" => { 
            required: ["name"], 
            optional: ["file", "header"], 
        },
        
        "pure" => {},
        
        "requirement" => { 
            required: ["id"], 
            optional: ["title"], 
        },
        "showinitializer" => {},
        "struct" => { 
            required: ["name"], 
            optional: ["file", "header"], 
        },
        "typedef" => { 
            attached: "declaration" {
                required: true, 
                until: newline, 
            },
        },
        "union" => { 
            required: ["name"], 
            optional: ["file", "header"], 
        },
        "var" => { 
            attached: "declaration" {
                required: true, 
                until: newline, 
            },
        },
        "vhdlflow" => { 
            optional: ["title"], 
        },
        
        
        
        
        
        "cond" => { 
            optional: ["label"], 
        },
        "copyright" => { 
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
        },
        "date" => { 
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
        },
        "showdate" => { 
            required: ["format"], 
            optional: ["datetime"], 
        },
        
        "details" => { 
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
        },
        "noop" => { 
            attached: "body" {
                required: true, 
                until: interrupt, 
            },
        },
        "raisewarning" => { 
            attached: "warning" {
                required: true, 
                until: newline, 
            },
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Warning), 
                    message: param!("warning")
                }
            ],
        },
        "else" => {},
        "elseif" => { 
            required: ["label"], 
        },
        "endcond" => {},
        "endif" => {},
        "exception" => { 
            required: ["exception"], 
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
        },
        "if" => { 
            required: ["label"], 
        },
        "ifnot" => { 
            required: ["label"], 
        },
        "invariant" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
        },
        
        
        "parblock" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("parblock is not supported")
                },
            ],
        },
        "endparblock" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("parblock is not supported")
                },
            ],
        },
        
        "sa" => { 
            attached: "references" {
                required: true,
                until: interrupt,
            },
        },
        "see" => { 
            attached: "references" {
                required: true,
                until: interrupt,
            },
        },
        "short" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
        },
        "since" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
        },
        "test" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
        },
        
        "version" => { 
            attached: "version" {
                required: true, 
                until: interrupt, 
            },
        },
        "xrefitem" => { 
            required: ["key", "heading", "title"], 
            attached: "body" {
                required: true, 
                until: interrupt, 
            },
        },
        "addindex" => { 
            attached: "body" {
                required: true, 
                until: newline, 
            },
        },
        "anchor" => { 
            required: ["name"], },
        "cite" => { 
            delimited: Options('\0', std::collections::HashMap::from([
                ("number", false), 
                ("shortauthor", false), 
                ("year", false), 
                ("nopar", false), 
                ("nocite", false)
            ])), 
            required: ["label"], 
        },
        "endlink" => {},
        "link" => { 
            required: ["object"], 
        },
        "ref" => { 
            required: ["id"], 
            script: vec![
                Action::InsertReference { 
                    entry_id: param!("id")
                }
            ],
        },
        "refitem" => { 
            required: ["name"], 
        },
        "satisfies" => { 
            required: ["id"], 
            optional: ["description"], 
            script: vec![
                Action::AddRelationship { 
                    kind: v!(ir::Relationship::predefined("doxy.satisfies")), 
                    entry_id: param!("id")
                }
            ],
        },
        "verifies" => { 
            required: ["id"], 
            optional: ["description"], 
            script: vec![
                Action::AddRelationship { 
                    kind: v!(ir::Relationship::predefined("doxy.verifies")), 
                    entry_id: param!("id")
                }
            ],
        },
        
        "secreflist" => {},
        "endsecreflist" => {},
        
        "tableofcontents" => { 
            delimited: Options(':', std::collections::HashMap::from([
                ("HTML", true), 
                ("LaTeX", true), 
                ("XML", true), 
                ("DocBook", true)
            ])), 
        },
        "section" => { 
            required: ["id"], 
            optional: ["title"], 
            script: vec![
                Action::InsertHeadline { 
                    id: param!("id"),
                    level: v!(2), 
                    content: param!("title")
                },
            ],
        },
        "subsection" => { 
            required: ["name"], 
            optional: ["title"], 
            script: vec![
                Action::InsertHeadline { 
                    id: param!("id"),
                    level: v!(3), 
                    content: param!("title")
                },
            ],
        },
        "subsubsection" => { 
            required: ["name"], 
            optional: ["title"], 
            script: vec![
                Action::InsertHeadline { 
                    id: param!("id"),
                    level: v!(4), 
                    content: param!("title")
                },
            ],
        },
        "paragraph" => { 
            required: ["name"], 
            optional: ["title"],
        },
        "subparagraph" => { 
            required: ["name"], 
            optional: ["title"], 
        },
        "subsubparagraph" => { 
            required: ["name"], 
            optional: ["title"], 
        },
        "dontinclude" => { 
            delimited: Options('\0', std::collections::HashMap::from([
                ("lineno", false), 
                ("strip", false), 
                ("nostrip", false)
            ])), 
            required: ["file"], 
        },
        "include" => { 
            delimited: Options('=', std::collections::HashMap::from([
                ("lineno", false), 
                ("doc", false), 
                ("local", false), 
                ("strip", false), 
                ("nostrip", false), 
                ("raise", true), 
                ("prefix", true)
            ])), 
            required: ["file"], 
        },
        "includelineno" => { 
            required: ["file"], 
        },
        "includedoc" => { 
            delimited: Options('=', std::collections::HashMap::from([
                ("raise", true), 
                ("prefix", true)
            ])), 
            required: ["file"], 
        },
        "line" => { 
            attached: "pattern" {
                required: true, 
                until: interrupt, 
            },
        },
        "skip" => { 
            attached: "pattern" {
                required: true, 
                until: interrupt, 
            },
        },
        "skipline" => { 
            attached: "pattern" {
                required: true, 
                until: interrupt, 
            },
        },
        "snippet" => { 
            delimited: Options('=', std::collections::HashMap::from([
                ("lineno", false), 
                ("trimleft", false), 
                ("doc", false), 
                ("local", false), 
                ("strip", false), 
                ("nostrip", false), 
                ("raise", true), 
                ("prefix", true)
            ])), 
            required: ["file", "id"], 
        },
        "snippetlineno" => { 
            required: ["file", "id"], 
        },
        "snippetdoc" => { 
            delimited: Options('=', std::collections::HashMap::from([
                ("raise", true), 
                ("prefix", true)
            ])), 
            required: ["file", "id"], 
        },
        "until" => { 
            attached: "pattern" {
                required: true, 
                until: interrupt, 
            },
        },
        
        
        
        
        "copydoc" => { 
            required: ["id"], 
        },
        "copybrief" => { 
            required: ["id"], 
        },
        "copydetails" => { 
            required: ["id"], 
        },
        
        "dot" => { 
            optional: ["caption", "size"], 
        },
        
        "msc" => { 
            optional: ["caption", "size"], 
        },
        "mermaid" => { 
            optional: ["caption", "size"], 
        },
        "startuml" => { 
            optional: ["caption", "size"], 
        },
        "dotfile" => { 
            required: ["file"], 
            optional: ["caption", "size"], 
        },
        "mscfile" => { 
            required: ["file"], 
            optional: ["caption", "size"], 
        },
        "diafile" => { 
            required: ["file"], 
            optional: ["caption", "size"], 
        },
        
        
        
        "enddot" => {},
        "endmsc" => {},
        "endmermaid" => {},
        "enduml" => {},
        "mermaidfile" => { 
            required: ["file"], 
            optional: ["caption", "size"], 
        },
        "plantumlfile" => { 
            required: ["file"], 
            optional: ["caption", "size"], 
        },

        // MARK: - Characteristic sections

        "tparam" => { 
            required: ["name"], 
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::PARAMETERS), 
                    value: param!("name"), 
                    description: param!("description"), 
                },
                
            ],
        },
        "post" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::POSTCONDITION), 
                    value: param!("name"), 
                    description: param!("description"), 
                },
                
            ],
        },
        "pre" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::PRECONDITION), 
                    value: param!("name"), 
                    description: param!("description"), 
                },
                
            ],
        },
        "result" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::RETURNS), 
                    value: param!("name"), 
                    description: param!("description"), 
                },
                
            ],
        },
        "return" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::PRECONDITION), 
                    value: param!("name"), 
                    description: param!("description"), 
                },
                
            ],
        },
        "returns" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::SetSectionAbstract { 
                    kind: v!(ir::CharacteristicSection::RETURNS), 
                    content: param!("description"), 
                },
                
            ],
        },
        "retval" => { 
            required: ["value"], 
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::RETURNS), 
                    value: param!("value"), 
                    description: param!("description"), 
                },
                
            ],
        },
        "throw" => { 
            required: ["exception"], 
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::THROWS), 
                    value: param!("exception"), 
                    description: param!("description"), 
                },
            ],
        },
        "param" => { 
            delimited: KeywordModifiers(&["in", "out", "inout"]), 
            required: ["name"], 
            optional: ["'-'"],
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::PARAMETERS), 
                    value: param!("name"), 
                    description: param!("description"), 
                },
                
            ],
        },
        "bug" => { 
            attached: "description" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::ISSUES), 
                    value: param!("name"), 
                    description: param!("description"), 
                },
                
            ],
        },
        "throws" => { 
            required: ["exception"],
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::THROWS), 
                    value: param!("exception"), 
                    description: param!("description"), 
                },
            ],
        },
        "todo" => { 
            attached: "description" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::AddSectionItem { 
                    kind: v!(ir::CharacteristicSection::predefined("todo")), 
                    value: param!("..."), 
                    description: param!("description"), 
                },
            ],
            
        },
        

        // MARK: - Admonitions

        "note" => { 
            attached: "body" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::InsertAdmonition { 
                    contents: param!("body"), 
                    kind: v!(ir::AdmonitionKind::Note)
                }
            ],
        },
        "important" => { 
            attached: "body" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::InsertAdmonition { 
                    contents: param!("body"), 
                    kind: v!(ir::AdmonitionKind::Important)
                }
            ],
        },
        "warning" => { 
            attached: "body" {
                required: true, 
                until: interrupt, 
            },
        },
        "remark" => { 
            attached: "body" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::InsertAdmonition { 
                    contents: param!("body"), 
                    kind: v!(ir::AdmonitionKind::Tip)
                }
            ],
        },
        "remarks" => { 
            attached: "body" {
                required: true,
                until: interrupt,
            },
            script: vec![
                Action::InsertAdmonition { 
                    contents: param!("body"), 
                    kind: v!(ir::AdmonitionKind::Tip)
                }
            ],
        },
        "attention" => { 
            attached: "body" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::InsertAdmonition { 
                    contents: param!("body"), 
                    kind: v!(ir::AdmonitionKind::Caution)
                }
            ],
        },
        "par" => { 
            attached: "body" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::InsertAdmonition { 
                    contents: param!("body"), 
                    kind: v!(ir::AdmonitionKind::Note)
                }
            ],
        },


        // MARK: - Rich-text directives

        "verbinclude" => { 
            required: ["file"], 
            script: vec![
                Action::InsertSnippet { 
                    source: param!("file"), 
                    language: v!(None),
                    snippet: v!(None)
                }
            ],
        },
        "verbatim" => {
            script: vec![
                Action::StartCodeBlock { language: v!(None) },
                Action::Diagnose { 
                    level: v!(ir::DiagnosticLevel::Warning), 
                    message: s!("Use mardown backticks (```) to start code block instead")
                }
            ],
        },
        "endverbatim" => {
            script: vec![
                Action::EndCodeBlock {  },
                Action::Diagnose { 
                    level: v!(ir::DiagnosticLevel::Warning), 
                    message: s!("Use mardown backticks (```) to end code block instead")
                }
            ],
        },
        "code" => { 
            optional: ["language"], 
            script: vec![
                Action::StartCodeBlock { language: param!("language") },
                Action::Diagnose { 
                    level: v!(ir::DiagnosticLevel::Warning), 
                    message: s!("Use mardown backticks (```) to start code block instead")
                }
            ],
        },
        "endcode" => {
            script: vec![
                Action::EndCodeBlock {  },
                Action::Diagnose { 
                    level: v!(ir::DiagnosticLevel::Warning), 
                    message: s!("Use mardown backticks (```) to end code block instead")
                }
            ],
        },

        "image" => { 
            delimited: Options(':', std::collections::HashMap::from([
                ("inline", false), 
                ("anchor", true)
            ])), 
            required: ["format", "file"], 
            optional: ["caption", "size"], 
            script: vec![
                Action::PushScope { 
                    name: s!("doxy.image")
                },
                Action::AddAllowedOutputBackend {
                    allowed: param!("format"),
                },
                Action::InsertImage { 
                    image_source: param!("file"), 
                    caption: param!("caption"),
                    inline: param!("inline"),
                    id: param!("anchor")
                },
                Action::PopScope { 
                    name: s!("doxy.image")
                },
            ],
        },
        "li" => { 
            attached: "body" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::InsertListItem { 
                    style: v!(ir::rich_text::ListStyle::Unordered),
                    content: param!("body")
                },
                Action::Diagnose { 
                    level: v!(ir::DiagnosticLevel::Warning), 
                    message: s!("Use mardown list items (- ...) for list items")
                }
            ],
        },
        "arg" => { 
            attached: "body" {
                required: true, 
                until: interrupt, 
            },
            script: vec![
                Action::InsertListItem { 
                    style: v!(ir::rich_text::ListStyle::Unordered),
                    content: param!("body")
                },
                Action::Diagnose { 
                    level: v!(ir::DiagnosticLevel::Warning), 
                    message: s!("Use mardown list items (- ...) for list items")
                }
            ],
        },
        "p" => { 
            required: ["codeword"],
            script: vec![
                Action::InsertCode { 
                    language: v!(None),
                    content: param!("codeword")
                },
            ],
        },
        "c" => { 
            required: ["codeword"],
            script: vec![
                Action::InsertCode { 
                    language: v!(None),
                    content: param!("codeword")
                },
            ],
        },
        "b" => { 
            required: ["word"],
            script: vec![
                Action::InsertStyled { 
                    text: param!("word"), 
                    style: v!(ir::rich_text::InlineTextStyle::Bold)
                }
            ],
        },
        "em" => { 
            required: ["word"],
            script: vec![
                Action::InsertStyled { 
                    text: param!("word"), 
                    style: v!(ir::rich_text::InlineTextStyle::Emphasized)
                }
            ],
        },
        "e" => { 
            required: ["word"],
            script: vec![
                Action::InsertStyled { 
                    text: param!("word"), 
                    style: v!(ir::rich_text::InlineTextStyle::Emphasized)
                }
            ],
        },
        "a" => { 
            required: ["word"],
            script: vec![
                Action::InsertStyled { 
                    text: param!("word"), 
                    style: v!(ir::rich_text::InlineTextStyle::Emphasized)
                }
            ],
        },

        "emoji" => { 
            required: ["name"], 
            script: vec![
                Action::InsertEmoji {
                    name: param!("name"), 
                }
            ],
        },

        // MARK: - Output format restrictions

        "htmlonly" => { 
            delimited: KeywordModifiers(&["block"]),
            script: vec![
                Action::SetParameter { 
                    argument: s!("format"), 
                    value: v!(ir::Value::String("html".into()))
                },
                Action::Script("doxygen.verbatim-into-output.start".into())
            ],
        },
        "endhtmlonly" => {
            script: vec![
                Action::SetParameter { 
                    argument: s!("format"), 
                    value: v!(ir::Value::String("html".into()))
                },
                Action::Script("doxygen.verbatim-into-output.end".into())
            ],
        },

        "htmlinclude" => { 
            delimited: KeywordModifiers(&["block"]), 
            required: ["file"], 
            script: vec![
                Action::SetParameter { 
                    argument: s!("format"), 
                    value: v!(ir::Value::String("html".into()))
                },
                Action::Script("doxygen.verbatim-into-output.start".into()),
                Action::EmbedFromFile {
                    language: v!(Some(ir::Language::HTML)),
                    source: param!("file")
                },
                Action::Script("doxygen.verbatim-into-output.end".into())
            ],
        },

        "docbookonly" => {
            script: vec![
                
            ],
        },
        "enddocbookonly" => {
            script: vec![
                
            ],
        },


        "latexonly" => {
            script: vec![
                
            ],
        },
        "endlatexonly" => {
            script: vec![
                Action::PopScope { 
                    name: s!("doxy.latexonly")
                },

                // Insert <docbookonly> into XML
                Action::PushScope { 
                    name: s!("doxy.latexonly.xml")
                },
                Action::SetScopeLanguage {
                    language: v!(ir::Language::XML)
                },
                Action::AddAllowedOutputBackend {
                    allowed: v!(backend::Format::DoxyXML),
                },
                Action::Embed {
                    language: v!(Some(ir::Language::XML)),
                    content: s!("</latexonly>")
                },
                Action::PopScope {
                    name: s!("doxy.latexonly.xml") 
                },
            ],
        },


        "manonly" => {
            script: vec![
                
            ],
        },
        "endmanonly" => {
            script: vec![
                
            ],
        },

        
        "latexinclude" => { 
            required: ["file"], 
            script: vec![
                Action::SetParameter { 
                    argument: s!("format"), 
                    value: v!(ir::Value::String("latex".into()))
                },
                Action::Script("doxygen.verbatim-into-output.start".into()),
                Action::EmbedFromFile {
                    language: v!(Some(ir::Language::HTML)),
                    source: param!("file")
                },
                Action::Script("doxygen.verbatim-into-output.end".into())
            ],
        },
        "rtfinclude" => { 
            required: ["file"], 
            script: vec![
                Action::SetParameter { 
                    argument: s!("format"), 
                    value: v!(ir::Value::String("rtf".into()))
                },
                Action::Script("doxygen.verbatim-into-output.start".into()),
                Action::EmbedFromFile {
                    language: v!(Some(ir::Language::RTF)),
                    source: param!("file")
                },
                Action::Script("doxygen.verbatim-into-output.end".into())
            ],
        },
        "maninclude" => { 
            required: ["file"], 
            script: vec![
                Action::SetParameter { 
                    argument: s!("format"), 
                    value: v!(ir::Value::String("man".into()))
                },
                Action::Script("doxygen.verbatim-into-output.start".into()),
                Action::EmbedFromFile {
                    language: v!(Some(ir::Language::ManPages)),
                    source: param!("file")
                },
                Action::Script("doxygen.verbatim-into-output.end".into())
            ],
        },
        "docbookinclude" => { 
            required: ["file"], 
            script: vec![
                Action::SetParameter { 
                    argument: s!("format"), 
                    value: v!(ir::Value::String("docbook".into()))
                },
                Action::Script("doxygen.verbatim-into-output.start".into()),
                Action::EmbedFromFile {
                    language: v!(Some(ir::Language::DocBook)),
                    source: param!("file")
                },
                Action::Script("doxygen.verbatim-into-output.end".into())
            ],
        },


        "rtfonly" => {
            script: vec![
                
            ],
        },
        "endrtfonly" => {
            script: vec![
                
            ],
        },

        "xmlonly" => {
            script: vec![
                Action::PushScope { name: s!("doxy.xmlonly") },
                Action::SetScopeLanguage {
                    language: v!(ir::Language::XML)
                },
                Action::AddAllowedOutputBackend {
                    allowed: v!(backend::Format::DoxyXML),
                },
            ],
        },
        "endxmlonly" => {
            script: vec![
                Action::PopScope { name: s!("doxy.xmlonly") },
            ],
        },
        "xmlinclude" => { 
            required: ["file"],
            script: vec![
                Action::PushScope { name: s!("doxy.xmlonly") },
                Action::SetScopeLanguage {
                    language: v!(ir::Language::XML)
                },
                Action::AddAllowedOutputBackend {
                    allowed: v!(backend::Format::DoxyXML),
                },
                Action::EmbedFromFile {
                    language: v!(ir::Language::XML),
                    source: param!("file")
                },
                Action::PopScope { name: s!("doxy.xmlonly") },
            ],
        },

        // MARK: - Doxygen built-ins

        "doxyconfig" => { 
            required: ["option"], 
            script: vec![
                Action::Builtin(DoxygenBuiltin::Doxyconfig)
            ],
        },

        "file" => { 
            optional: ["name"], 
            script: vec![
                Action::Builtin(DoxygenBuiltin::File)
            ],
        },
        "fileinfo" => { 
            delimited: Options('\0', std::collections::HashMap::from([
                ("name", false), 
                ("extension", false), 
                ("filename", false), 
                ("directory", false), 
                ("full", false)
            ])),
            script: vec![
                Action::Builtin(DoxygenBuiltin::FileInfo)
            ],
        },
        "lineinfo" => {
            script: vec![
                Action::Builtin(DoxygenBuiltin::LineInfo)
            ],
        },

        // MARK: - Graph commands (currently not implemented)
        
        "callgraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Call graphs are not supported")
                },
            ],
        },
        "hidecallgraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Call graphs are not supported")
                },
            ],
        },
        "callergraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Call graphs are not supported")
                },
            ],
        },
        "hidecallergraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Call graphs are not supported")
                },
            ],
        },

        "includegraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Include graphs are not supported")
                },
            ],
        },
        "hideincludegraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Include graphs are not supported")
                },
            ],
        },
        "includedbygraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Include graphs are not supported")
                },
            ],
        },
        "hideincludedbygraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Include graphs are not supported")
                },
            ],
        },
        "directorygraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Directory graphs are not supported")
                },
            ],
        },
        "hidedirectorygraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Directory graphs are not supported")
                },
            ],
        },
        "collaborationgraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Callaboration graphs are not implemented yet")
                },
            ],
        },
        "hidecollaborationgraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Callaboration graphs are not implemented yet")
                },
            ],
        },
        "inheritancegraph" => { 
            delimited: Options('\0', std::collections::HashMap::from([
                ("YES", false), 
                ("NO", false)
            ])), 
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Inheritance graphs are not implemented yet")
                },
            ],
        },
        "hideinheritancegraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Inheritance graphs are not implemented yet")
                },
            ],
        },
        "groupgraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Group graphs are not implemented yet")
                },
            ],
        },
        "hidegroupgraph" => {
            script: vec![
                Action::Diagnose {
                    level: v!(ir::DiagnosticLevel::Error),
                    message: s!("Group graphs are not implemented yet")
                },
            ],
        },

    }
}

pub fn _scripts() -> HashMap<&'static str, Script<'static, DoxygenDirective<'static>>> {
    HashMap::from([
        (
            "doxygen.verbatim-into-output.start",
            vec![
                Action::PushScope { name: t!("doxy.formatonly.$format") },

                // Insert <htmlonly> into XML
                Action::PushScope { name: s!("doxy.formatonly.xml") },
                Action::SetScopeLanguage {
                    language: v!(ir::Language::XML)
                },
                Action::AddAllowedOutputBackend {
                    allowed: v!(backend::Format::DoxyXML),
                },
                Action::Embed {
                    language: v!(Some(ir::Language::XML)),
                    content: t!("<${format}only>")
                },
                Action::PopScope { name: s!("doxy.formatonly.xml") },

                Action::SetScopeLanguage {
                    language: param!("format")
                },
                Action::AddAllowedOutputBackend {
                    allowed: param!("format"),
                },
                Action::AddAllowedOutputBackend {
                    allowed: v!(backend::Format::DoxyXML),
                },
            ]
        ),

        (
            "doxygen.verbatim-into-output.end",
            vec![
                Action::PopScope { name: t!("doxy.${format}only") },

                // Insert <htmlonly> into XML
                Action::PushScope { name: s!("doxy.formatonly.xml")},
                Action::SetScopeLanguage { 
                    language: v!(ir::Language::XML) 
                },
                Action::AddAllowedOutputBackend {
                    allowed: v!(backend::Format::DoxyXML),
                },
                Action::Embed {
                    language: v!(Some(ir::Language::XML)),
                    content: t!("</${format}only>")
                },
                Action::PopScope { name: s!("doxy.formatonly.xml") },
            ]
        )
    ])
}