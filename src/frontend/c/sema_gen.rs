#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

use super::sema::{
    Array, ArrayQualifier, AttachedExpression, Attribute, CType, Container,
    DeclarationQualifier, Enum, EnumCase, Expression, Function, FunctionQualifier, Identifier,
    IncludedHeader, MSCallModifier, MSDeclModifier, MSPointerModifier, MacroDefinition,
    Modifier, NamedType, Parameter, Pointer, PointerQualifier, PrimitiveType, RawSpelling,
    SizeModifier, StandardAttribute, Storage, Symbol, TypeDecl, TypeDefinition,
    TypeQualifier, Variable,
};

use crate::{compiler::{
    diagnostics::{
        self, Diagnostic, DiagnosticReporter, LocatedDiagnostic, LocationAttachableDiagnostic
    }, strings::Source
}, frontend::c};

use std::collections::HashMap;
use std::str::FromStr;
use std::{error, fmt};
use clap::builder::Str;
use serde::de;
use strum_macros::Display;
use log::{
    debug, 
    // error,
    // warn
};
use tree_sitter::{
    Node,
};

// Tree-sitter node names originate from static C strings in generated parser,
// hence, when emitting diagnostics containing their names, we can use a &'static str.
// Else, we need to allocate a String in order for the error type to have a static lifetime.
    
#[derive(Debug, thiserror::Error)]
pub enum ReportMessage<E: error::Error + 'static> {
    #[error("Failed to read from source: {0}")]
    Unreadable(#[from] #[source] E), 
    // The #[from] attribute always implies that the same field is #[source], 
    // so you don’t ever need to specify both attributes.

    #[error("Declaration <{0}> has no 'type' field")]
    DeclMissingType(&'static str),
    
    #[error("Primitive type '{0}' is unknown")]
    UnknownPrimitiveType(String),
    
    #[error("Leaf declarator <{0}> is unknown")]
    UnknownLeafDeclarator(&'static str),

    #[error("Type is not representable")]
    UnrepresentableType,

    #[error("<{0}> is missing both identifier and body")]
    UnnamedEmptyType(&'static str),

    #[error("<type_qualifier> is missing child, cannot create decl qualifier")]
    MissingTypeQualifierChild,

    #[error("Non-leaf declarator <{0}> is missing inner declarator")]
    MissingInnerDeclarator(&'static str),

    #[error("<{0}> is missing a declarator, assuming anonymous variable. If this is not intended, please file a bug report.")]
    MissingAnyDeclarators(&'static str),

    #[error("declspec modifier is missing content")]
    MissingDeclSpecModifierContent,

    #[error("Symbol has more than one initializer")]
    DuplicateInitializer,

    #[error("Variable with initializer must not have bitfield clause")]
    InitializerAndBitfieldClause,

    #[error("Declarator <{0}> is missing value")]
    MissingValue(&'static str),

    #[error("Attribute is missing name")]
    MissingStandardAttributeIdentifier,

    #[error("Missing preprocessor ifdef identifier")]
    PreprocessorMissingIdentifier,

    #[error("Type definition is missing identifier")]
    MissingTypedefIdentifier,

    #[error("Missing argument list")]
    MissingArgumentList,

    #[error("Expression is missing operator")]
    MissingOperator,

    #[error("Expression is missing an operand")]
    MissingOperand,

    #[error("Bitfield clause is missing bit width expression")]
    MissingBitfieldClauseExpr,

    #[error("Enum case is missing name")]
    MissingEnumCaseName,

    #[error("Function parameter has unknown format")]
    WeirdFunctionParameter,

    #[error("Field in field declaration does not yield variable")]
    NonVariableMember,

    #[error("Unable to create function parameter from <{0}>")]
    UnsupportedFunctionParameterDecl(&'static str),

    #[error("Enum has unknown underlying type '{0}'")]
    UnknownUnderlyingEnumType(String),

    #[error("Declarator <{0}> is unknown")]
    UnknownDeclarator(&'static str),

    #[error("Decl qualifier '{0}' is unknown")]
    UnknownDeclQualifier(String),

    #[error("Attribute specifier is unknown")]
    UnknownAttributeSpecifier,

    #[error("Unknown field decl list item <{0}>")]
    UnknownFieldDeclListItem(&'static str),

    #[error("Call modifier '{0}' is unknown")]
    UnknownCallModifier(String),

    #[error("Unable to read argument list from attribute: {0}")]
    CorruptedAttributeArgumentList(E),

    #[error("Binary operator '{0}' is unknown")]
    UnknownBinaryOperator(String),
    
    #[error("Unary operator '{0}' is unknown")]
    UnknownUnaryOperator(String),

    #[error("Preprocessor expression <{0}> is unknown")]
    UnknownPreprocessorExpr(&'static str),

    #[error("Number literal '' is invalid")]
    InvalidNumberLiteral(#[source] <usize as FromStr>::Err),

    #[error("Character literal '' is invalid")]
    InvalidCharacterLiteral(#[source] <char as FromStr>::Err),

    #[error("Preprocessor invocation is missing directive")]
    MissingDirective,

    #[error("<preproc_if> is missing condition")]
    MissingCondition,

    #[error("Preprocessor condition not met and alternative <{0}> is unknown")]
    MissingAlternative(&'static str),

    #[error("Decl has multiple types")]
    DuplicateTypes,

    #[error("#include is missing \"local\" or <global> header path")]
    PreprocessorMissingIncludePath,

    #[error("#include is missing local header path")]
    PreprocessorMissingIncludePathLocal,

    #[error("AST node <{0}> has unknown child node <{1}>")]
    UnknownChildNode(&'static str, &'static str),

    #[error("AST node has unknown child token '{0}'")]
    UnknownChildToken(String),

    #[error("Conditional preprocessor directive '{0}' is unknown and cannot be evaluated")]
    UnknownConditionalPreprocessorDirective(String),

    #[error("<type_identifier> must only occur in typedefs")]
    UnexpectedTypeIdentifier,

    #[error("Qualifier '{0}' is not expected to occur in <{1}>")]
    UnexpectedQualifier(String, &'static str),

    #[error("Qualifier '{0}' is not applicable, only type, decl, or function qualifiers are allowed")]
    InapplicableQualifier(String),

    #[error("Qualifier '{0}' is not applicable behind pointer declarator")]
    InapplicablePointerQualifier(String),

    #[error("Qualifier '{0}' is not applicable in array declarator")]
    InapplicableArrayQualifier(String),

    #[error("Qualifier '{0}' is not applicable to this type")]
    InapplicableTypeQualifier(String),

    #[error("Preprocessor calls {0} are ignored")]
    PreprocessorCallIngored(&'static str),

    #[error("Conditional preprocessor content with unsatisfied condition '{0}' has been ignored")]
    UnsatisfiedPreprocessorCondition(String),

    #[error("AST scope child <{0}> is not known to occur in this position")]
    ScopeChildIngored(&'static str),

    #[error("Unable to create symbols from <{0}> conditional preprocessor content: {1}")]
    ConditionalPreprocessorContentResolvingFailure(&'static str, Box<SemaGenError<E>>),

    #[error("Unable to create symbol from AST of <{0}>: {1}")]
    SemGenFailed(&'static str, Box<SemaGenError<E>>),
}

pub type SemaGenError<E: error::Error> = LocatedDiagnostic<ReportMessage<E>, diagnostics::Location, diagnostics::Location>;

impl<'a, E: error::Error> diagnostics::Diagnostic for ReportMessage<E> {
    fn severity(&self) -> diagnostics::Severity {
        match self {
            Self::UnsatisfiedPreprocessorCondition(_)
            => diagnostics::Severity::Info,
            Self::UnexpectedQualifier(_, _) |
            Self::InapplicableQualifier(_) |
            Self::InapplicablePointerQualifier(_) |
            Self::InapplicableArrayQualifier(_) |
            Self::PreprocessorCallIngored(_) |
            Self::ScopeChildIngored(_) |
            Self::UnknownFieldDeclListItem(_)
            => diagnostics::Severity::Warning,
            _ => diagnostics::Severity::Error
        }
    }

    fn is_internal(&self) -> bool { true }
}

trait NodeSourceExtension {
    fn string<S: Source>(&self, source: S) -> Result<String, SemaGenError<S::StrError>>;
    fn slice<S: Source>(&self, source: S) -> Result<S::AsSlice, SemaGenError<S::StrError>>;
    fn str<S: Source>(&self, source: S) -> Result<S::AsStrSlice, SemaGenError<S::StrError>>;
}

impl NodeSourceExtension for tree_sitter::Node<'_> {
    fn string<S: Source>(&self, source: S) -> Result<String, SemaGenError<S::StrError>> {
        source.string(self.range())
            .map_err(|e| ReportMessage::Unreadable(e).at(self.range()))
    }

    fn slice<S: Source>(&self, source: S) -> Result<S::AsSlice, SemaGenError<S::StrError>> {
        source.slice(self.range())
            .map_err(|e| ReportMessage::Unreadable(e).at(self.range()))
    }

    fn str<S: Source>(&self, source: S) -> Result<S::AsStrSlice, SemaGenError<S::StrError>> {
        source.str(self.range())
            .map_err(|e| ReportMessage::Unreadable(e).at(self.range()))
    }
}

pub struct Config {
    decay_arrays_to_pointers: bool,
    ingore_header_guard_defines: bool,
    preprocessor_defines: HashMap<String, isize>,
}

impl Config {
    pub fn default() -> Self {
        Self {
            decay_arrays_to_pointers: true,
            ingore_header_guard_defines: true,
            preprocessor_defines: HashMap::<String, isize>::from([("DOXYGEN".into(), 1 as isize)]),
        }
    }
}

pub trait Recorder<E> {
    fn record_include(&mut self, header: IncludedHeader) -> Result<bool, E>;
    fn record_symbol(&mut self, symbol: Symbol) -> Result<bool, E>;
    fn record_comment(&mut self, comment: &str) -> Result<bool, E>;
    fn finish(&mut self) -> Result<(), E>;
}

impl Recorder<()> for Vec<Symbol> {
    fn record_include(self: &mut Self, _header: IncludedHeader) -> Result<bool, ()> {
        Ok(false)
    }

    fn record_symbol(self: &mut Self, symbol: Symbol) -> Result<bool, ()> {
        self.push(symbol);
        Ok(true)
    }

    fn record_comment(self: &mut Self, _comment: &str) -> Result<bool, ()> {
        Ok(false)
    }

    fn finish(self: &mut Self) -> Result<(), ()> {
        Ok(())
    }
}

impl Recorder<()> for Vec<IncludedHeader> {
    fn record_include(self: &mut Self, header: IncludedHeader) -> Result<bool, ()> {
        self.push(header);
        Ok(true)
    }

    fn record_symbol(self: &mut Self, _symbol: Symbol) -> Result<bool, ()> {
        Ok(false)
    }

    fn record_comment(self: &mut Self, _comment: &str) -> Result<bool, ()> {
        Ok(false)
    }

    fn finish(self: &mut Self) -> Result<(), ()> {
        Ok(())
    }
}

struct _Range(tree_sitter::Range);

impl fmt::Display for _Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}, {}] - [{}, {}]",
            self.0.start_point.row,
            self.0.start_point.column,
            self.0.end_point.row,
            self.0.end_point.column
        )
    }
}

fn args_from_ast<S: Source>(
    argumentList: Node,
    source: S,
) -> Result<Vec<RawSpelling>, SemaGenError<S::StrError>> {
    argumentList
        .named_children(&mut argumentList.walk())
        .map(|s| s.string(source))
        .collect()
}

#[derive(Display)]
pub enum SemaGenError1<SourceError: error::Error> {
    readingFromSourceFailed(SourceError),
    languageError,
    declMissingType,
    unknownPrimitiveType,
    unsupportedCType,
    unnamedEmptyType,
    missingTypeQualifierChild,
    missingInnerDeclarator,
    duplicateInitializer,
    missingValue,
    standardAttributeMissingIdentifier,
    missingIdentifier,
    missingArgumentList,
    missingOperator,
    missingOperand,
    weirdFunctionParameter,
    unsupportedFunctionParameterDecl,
    unknownDeclarator,
    unknownDeclQualifier,
    corruptedArgumentList,
    unknownBinaryOperator,
    unknownUnaryOperator,
    unknownPreprocessorExpr,
    invalidNumberLiteral(<usize as FromStr>::Err),
    invalidCharacterLiteral(<char as FromStr>::Err),
    missingDirective,
    missingCondition,
    missingAlternative,
    parserInconsistency,
    parsingFailed,
}

// impl<SourceError: error::Error> From<SourceError> for SemaGenError<SourceError> {
//     fn from(e: SourceError) -> Self {
//         Self::readingFromSourceFailed(e)
//     }
// }

fn evaluatePreprocessorExpression<S: Source>(
    ast: Node,
    source: S,
    config: &Config,
) -> Result<isize, SemaGenError<S::StrError>> {
    // _preproc_expression: $ => choice(
    //   $.identifier,
    //   alias($.preproc_call_expression, $.call_expression),
    //   $.number_literal,
    //   $.char_literal,
    //   $.preproc_defined,
    //   alias($.preproc_unary_expression, $.unary_expression),
    //   alias($.preproc_binary_expression, $.binary_expression),
    //   alias($.preproc_parenthesized_expression, $.parenthesized_expression),
    // ),
    match ast.kind() {
        "identifier" => {
            let key = ast.string(source)?;

            Ok(config
                .preprocessor_defines
                .get(&key)
                .map_or_else(|| 0, |&v| v))
        }
        "call_expression" => {
            debug!("preprocessor call expr not supported, evaluating to false");
            Ok(0)
        }
        "number_literal" => isize::from_str(&ast.string(source)?)
            .map_err(|e| ReportMessage::InvalidNumberLiteral(e).at(ast.range())),
        "char_literal" => {
            let mut value: isize = 0;
            let mut iter = ast.walk();
            for char in ast.named_children(&mut iter) {
                let c = char::from_str(&char.string(source)?)
                    .map_err(|e| ReportMessage::InvalidCharacterLiteral(e).at(ast.range()))?;
                value += c as u32 as isize;
            }

            Ok(value)
        }
        "preproc_defined" => {
            let Some(identifier) = ast.named_child(0) else {
                return Err(ReportMessage::PreprocessorMissingIdentifier.at(ast.range()));
            };

            Ok(config
                .preprocessor_defines
                .contains_key(&identifier.string(source)?)
                .into())
        }
        "unary_expression" => {
            let operator = ast
                .child_by_field_name("operator")
                .ok_or(ReportMessage::MissingOperator.at(ast.range()))?;

            let operand = ast
                .child_by_field_name("argument")
                .ok_or(ReportMessage::MissingOperand.at(ast.range()))?;

            let op = operator.str(source)?;
            let operand = evaluatePreprocessorExpression(operand, source, config)?;

            match op.as_ref() {
                "!" => Ok((operand == 0).into()),
                "~" => Ok((!operand).into()),
                "-" => Ok((-operand).into()),
                "+" => Ok(operand),
                _ => {
                    // error!(
                    //     "unknown unary operator '{}' {}",
                    //     op.as_ref(),
                    //     _Range(operator.range())
                    // );
                    Err(ReportMessage::UnknownUnaryOperator(op.as_ref().to_string()).at(operator.range()))
                }
            }
        }
        "binary_expression" => {
            let operator = ast
                .child_by_field_name("operator")
                .ok_or(ReportMessage::MissingOperator.at(ast.range()))?;

            let left = ast
                .child_by_field_name("left")
                .ok_or(ReportMessage::MissingOperand.at(ast.range()))?;

            let right = ast
                .child_by_field_name("right")
                .ok_or(ReportMessage::MissingOperand.at(ast.range()))?;

            let _op = operator.str(source)?;
            let op = _op.as_ref();

            let left = evaluatePreprocessorExpression(left, source, config)?;
            let right = evaluatePreprocessorExpression(right, source, config)?;

            match op {
                "+" => Ok(left + right),
                "-" => Ok(left - right),
                "*" => Ok(left * right),
                "/" => Ok(left / right),
                "%" => Ok(left % right),
                "||" => Ok(if left == 0 { right } else { 1 }),
                "&&" => Ok(if left == 0 { 0 } else { right }),
                "|" => Ok(left | right),
                "^" => Ok(left ^ right),
                "&" => Ok(left & right),
                "==" => Ok(if left == right { left } else { 0 }),
                "!=" => Ok(if left == right { 0 } else { 1 }),
                ">" => Ok((left > right).into()),
                ">=" => Ok((left >= right).into()),
                "<=" => Ok((left <= right).into()),
                "<" => Ok((left < right).into()),
                "<<" => Ok(left << right),
                ">>" => Ok(left >> right),
                _ => {
                    // error!("uknown binary operator '{op}' {}", _Range(operator.range()));
                    Err(ReportMessage::UnknownBinaryOperator(op.to_string()).at(operator.range()))
                }
            }
        }
        "parenthesized_expression" => evaluatePreprocessorExpression(
            ast.named_child(0)
                .ok_or_else(|| ReportMessage::MissingOperand.at(ast.range()))?,
            source,
            config,
        ),
        _ => {
            // error!(
            //     "unknown preprocessor expression <{}> {}",
            //     ast.kind(),
            //     _Range(ast.range())
            // );
            Err(ReportMessage::UnknownPreprocessorExpr(ast.kind()).at(ast.range()))
        }
    }
}

fn symbolizePreprocessorConditionalBranch<S: Source, R: Recorder<E>, E>(
    ast: Node,
    source: S,
    config: &Config,
    recorder: &mut R,
    diags: &impl DiagnosticReporter,
) -> Result<(), SemaGenError<S::StrError>> {
    let mut iter = ast.walk();
    for child in ast.named_children(&mut iter).skip(1) {
        match child.kind() {
            "preproc_elif" | "preproc_else" => continue,
            _ => {}
        }

        match gen_sema(child, source, config, recorder, diags) {
            Err(e) => return Err(e),
            _ => {}
        }
    }

    Ok(())
}

fn symbolizePreprocessorConditionalContent<S: Source, R: Recorder<E>, E>(
    ast: Node,
    source: S,
    config: &Config,
    recorder: &mut R,
    diags: &impl DiagnosticReporter,
) -> Result<(), SemaGenError<S::StrError>> {
    let Some(condition) = ast.child_by_field_name("condition") else {
        // error!(
        //     "internal inconsistency: <preproc_if> is missing 'condition' field {}",
        //     _Range(ast.range())
        // );
        return Err(ReportMessage::MissingCondition.at(ast.range()));
    };

    if evaluatePreprocessorExpression(condition, source, config)? != 0 {
        symbolizePreprocessorConditionalBranch(ast, source, config, recorder, diags)
    } else {
        let Some(alternative) = ast.child_by_field_name("alternative") else {
            debug!("conditional preprocessor content has no alternative");
            return Ok(());
        };

        match alternative.kind() {
            "preproc_elif" => {
                symbolizePreprocessorConditionalContent(alternative, source, config, recorder, diags)
            }
            "preproc_else" => symbolizePreprocessorConditionalBranch(ast, source, config, recorder, diags),
            _ => {
                // error!(
                //     "preprocessor condition not met, but alternative is missing {}",
                //     _Range(ast.range())
                // );
                Err(ReportMessage::MissingAlternative(alternative.kind()).at(alternative.range()))
            }
        }
    }
}

pub fn symbolizePreprocessorIfDefinedConditionalContent<S: Source, R: Recorder<E>, E>(
    ast: Node,
    source: S,
    config: &Config,
    recorder: &mut R,
    diags: &impl DiagnosticReporter,
) -> Result<(), SemaGenError<S::StrError>> {
    let Some(directive) = ast.child(0) else {
        // error!(
        //     "preprocessor invocation is missing directive {}",
        //     _Range(ast.range())
        // );
        return Err(ReportMessage::MissingDirective.at(ast.range()));
    };

    let _s = directive.str(source)?;
    let s = _s.as_ref();
    let Some(identifier_node) = ast.child_by_field_name("name") else {
        // error!(
        //     "internal inconsistency: preproc_ifdef is missing 'name' field {}",
        //     _Range(ast.range())
        // );
        return Err(ReportMessage::PreprocessorMissingIdentifier.at(directive.range()));
    };

    let identifier = identifier_node.str(source)?;

    let mut conditionMet = config
        .preprocessor_defines
        .contains_key(identifier.as_ref());

    if s.ends_with("ifdef") {
        debug!("encountered ifdef");
    } else if s.ends_with("ifndef") {
        debug!("encountered ifndef");
        conditionMet = !conditionMet;
    } else {
        // error!(
        //     "cannot evaluate unknown preprocessor directive: '{s}' {}",
        //     _Range(directive.range())
        // );
        diags.diagnose(
            ReportMessage::<S::StrError>::UnknownConditionalPreprocessorDirective(s.to_string())
                .at(identifier_node.range())
        );
    }

    if conditionMet {
        gen_sema(ast, source, config, recorder, diags)
    } else {
        debug!(
            "ignoring conditional preprocessor directive '{s} {} {}', condition not met",
            identifier.as_ref(),
            _Range(directive.range())
        );

        diags.diagnose(
            ReportMessage::<S::StrError>::UnsatisfiedPreprocessorCondition((s.to_string() + " ") + identifier.as_ref())
                .at(directive.range())
        );

        Ok(())
    }
}

pub fn gen_sema<S: Source, R: Recorder<E>, E>(
    ast: Node,
    source: S,
    config: &Config,
    recorder: &mut R,
    diags: &impl DiagnosticReporter
) -> Result<(), SemaGenError<S::StrError>> {
    if ast.is_named() {
        debug!("symbolizing <{s}>", s = ast.kind());
    } else {
        debug!("symbolizing token '{s}'", s = ast.kind());
    }

    let mut cursor = ast.walk();
    for child in ast.children(&mut cursor) {
        if !child.is_named() {
            // debug!("ignoring <{s}>", s = child.kind());
            diags.diagnose(
                ReportMessage::<S::StrError>::ScopeChildIngored(child.kind())
                    .at(child.range())
            );
            continue;
        }

        match child.kind() {
            "preproc_include" => {
                let Some(path) = child.child_by_field_name("path") else {
                    // error!(
                    //     "<preproc_include> is missing 'path' field {}",
                    //     _Range(child.range())
                    // );
                    diags.diagnose(
                        ReportMessage::<S::StrError>::PreprocessorMissingIncludePath
                            .at(child.range())
                        );
                    continue;
                };

                match path.kind() {
                    "system_lib_string" => {
                        let s = path.string(source)?;
                        _ = recorder.record_include(IncludedHeader::system(s[1..s.len() - 1].into()));
                    }
                    "string_literal" => {
                        let Some(s) = path.child(1) else {
                            // error!("#include is missing path {}", _Range(path.range()));
                            diags.diagnose(
                                ReportMessage::<S::StrError>::PreprocessorMissingIncludePathLocal
                                    .at(path.range())
                                );
                            continue;
                        };

                        _ = recorder.record_include(IncludedHeader::local(s.string(source)?));
                    }
                    _ => {}
                }
            }
            "preproc_if" => {
                match symbolizePreprocessorConditionalContent(child, source, config, recorder, diags) {
                    Err(e) => 
                        diags.diagnose(
                            ReportMessage::ConditionalPreprocessorContentResolvingFailure(child.kind(), Box::new(e))
                        ),
                    // error!(
                    //     "could not symbolize conditional preprocessor content: {e} {}",
                    //     _Range(child.range())
                    // ),
                    _ => {}
                }
            }
            "preproc_ifdef" => {
                match symbolizePreprocessorIfDefinedConditionalContent(
                    child, source, config, recorder, diags
                ) {
                    Err(e) => diags.diagnose(
                            ReportMessage::ConditionalPreprocessorContentResolvingFailure(child.kind(), Box::new(e))
                        ),
                    // Err(e) => error!(
                    //     "could not symbolize conditional preprocessor content: {e} {}",
                    //     _Range(child.range())
                    // ),
                    _ => {}
                }
            }
            "preproc_def" | "preproc_function_def" => {
                match symbolizeMacroDefintion(child, source, config, recorder, diags) {
                    Err(e) => {
                        diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                    },
                    _ => {}
                }
            }
            "declaration" | "function_definition" => {
                debug!("encountered decl");
                match symbolsInDecl(child, source, config, false, recorder, diags) {
                    Err(e) => {
                        diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                    },
                    _ => {}
                }
            }
            "struct_specifier" => {
                match containerFromAST(child, source, config, recorder, diags) {
                    Ok(_struct) => {
                        // If this fails, we cannot do anything..
                        _ = recorder.record_symbol(Symbol::structure(_struct));
                    }
                    Err(e) => {
                        diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                    },
                }
            }
            "union_specifier" => {
                match containerFromAST(child, source, config, recorder, diags) {
                    Ok(_union) => {
                        // If this fails, we cannot do anything..
                        _ = recorder.record_symbol(Symbol::union(_union));
                    }
                    Err(e) =>  {
                        diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                    },
                }
            }
            "enum_specifier" => {
                match enumerationFromAST(child, source, config, recorder, diags) {
                    Ok(_enum) => {
                        // If this fails, we cannot do anything..
                        _ = recorder.record_symbol(Symbol::enumeration(_enum));
                    }
                    Err(e) =>  {
                        diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                    },
                }
            }
            "type_definition" => {
                debug!("encountered typedef");
                match symbolsInDecl(child, source, config, true, recorder, diags) {
                    Err(e) =>  {
                        diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                    },
                    _ => {}
                }
            }
            "comment" => {
                // recorder.record_comment(child.slice(source)?);
            }
            _ => {
                debug!(
                    "ignoring scope child <{}> {}",
                    child.kind(),
                    _Range(child.range())
                );
            }
        }
    }

    Ok(())
}

pub fn symbolizeMacroDefintion<S: Source, R: Recorder<E>, E>(
    ast: Node,
    source: S,
    config: &Config,
    recorder: &mut R,
    diags: &impl DiagnosticReporter
) -> Result<(), SemaGenError<S::StrError>> {
    let name = ast
        .child_by_field_name("name")
        .ok_or(ReportMessage::PreprocessorMissingIdentifier.at(ast.range()))?
        .string(source)?;

    if config.ingore_header_guard_defines && name.ends_with("_H") {
        debug!("ignoring header guard definition");
        return Ok(());
    }

    let mut parameters = Vec::<Parameter<Identifier>>::new();

    if let Some(params) = ast.child_by_field_name("parameters") {
        let mut iter = params.walk();
        for child in params.children(&mut iter) {
            let s = child.string(source)?;
            if child.is_named() {
                let true = child.kind() == "identifier" else {
                    // error!(
                    //     "<preproc_params has unknown child <{}> {}",
                    //     child.kind(),
                    //     _Range(child.range())
                    // );
                    diags.diagnose(
                        ReportMessage::<S::StrError>::UnknownChildNode(ast.kind(), child.kind())
                            .at(child.range())
                    );
                    continue;
                };

                parameters.push(Parameter::regular(s));
            } else {
                let true = s == "..." else {
                    // error!(
                    //     "<preproc_params has unknown child '{}' {}",
                    //     child.kind(),
                    //     _Range(child.range())
                    // );
                    diags.diagnose(
                        ReportMessage::<S::StrError>::UnknownChildNode(ast.kind(), child.kind())
                            .at(child.range())
                    );
                    continue;
                };

                parameters.push(Parameter::variadic(None));
            }
        }
    }

    _ = recorder.record_symbol(Symbol::macroDefintion(MacroDefinition {
        name: name,
        parameters: parameters,
        value: ast
            .child_by_field_name("value")
            .map(|v| v.string(source))
            .transpose()?,
    }));

    Ok(())
}

pub fn symbolsInDecl<S: Source, R: Recorder<E>, E>(
    ast: Node,
    source: S,
    config: &Config,
    isTypeDefinition: bool,
    recorder: &mut R,
    diags: &impl DiagnosticReporter,
) -> Result<(), SemaGenError<S::StrError>> {
    // declaration: $ => seq(
    //   $._declaration_specifiers,
    //   commaSep1(field('declarator', choice(
    //     seq(
    //       optional($.ms_call_modifier),
    //       $._declaration_declarator,
    //       optional($.gnu_asm_expression),
    //     ),
    //     $.init_declarator,
    //   ))),
    //   ';',
    // ),

    let mut modifiers = Vec::<Modifier>::new();
    let mut typeQualifiers = Vec::<TypeQualifier>::new();
    let mut declarationQualifiers = Vec::<DeclarationQualifier>::new();
    let mut functionQualifiers = Vec::<FunctionQualifier>::new();
    let mut declarators = Vec::<Node>::new();
    let mut _type: Option<CType> = None;

    if isTypeDefinition {
        if let Some(first) = ast.child(0) {
            if let Ok(name) = first.string(source) {
                if name == "__extension__" {
                    declarationQualifiers.push(DeclarationQualifier::extension);
                }
            }
        }
    }

    for i in 0..ast.child_count() {
        let Some(child) = ast.child(i) else {
            continue;
        };

        let fieldName = ast.field_name_for_child(i as u32);

        if let Some(fieldName) = fieldName {
            // debug!("child {i} has field name '{fieldName}'");

            match fieldName {
                "declarator" => {
                    // debug!("found declarator");
                    declarators.push(child);
                }
                "type" => {
                    debug!("found type: <{}>", child.kind());
                    if _type.is_some() {
                        // error!(
                        //     "parser inconsistency: duplicate type {}",
                        //     _Range(child.range())
                        // );
                        return Err(ReportMessage::DuplicateTypes.at(child.range()));
                    }

                    // We set type qualifiers below once we have collected all of them
                    _type = Some(CType::from_ast(
                        child,
                        source,
                        Vec::new(),
                        config,
                        recorder,
                        diags,
                    )?);

                    if _type.is_none() {
                        // error!(
                        //     "could not create C type from AST: <{}> {}",
                        //     child.kind(),
                        //     _Range(child.range())
                        // );
                        return Err(ReportMessage::DeclMissingType(child.kind()).at(child.range()));
                    }
                }
                _ => (),
            };
        } else if child.is_named() {
            let name = child.kind();
            debug!("child {i} is of type <{name}>");

            match name {
                "storage_class_specifier" => {
                    let Ok(s) = child.string(source) else {
                        continue;
                    };

                    let Ok(storage) = Storage::from_str(&s) else {
                        debug!("could not create modifier from <storage_class_specifier>");
                        continue;
                    };

                    debug!("found modifier {storage}");
                    modifiers.push(Modifier::storage(storage));
                }
                "attribute_specifier" => {
                    // attribute_specifier: $ => seq(
                    //   choice('__attribute__', '__attribute'),
                    //   '(',
                    //   $.argument_list,
                    //   ')',
                    // ),

                    if let Some(argumentList) = child.named_child(0) {
                        let args= args_from_ast(argumentList, source)?;
                        // else {
                            // error!(
                            //     "could not create attribute args from AST {}, ignoring",
                            //     _Range(child.range())
                            // );
                        //     return Err(ReportMessage::CorruptedAttributeArgumentList());
                        // };
                        let attribute = Attribute { arguments: args };
                        debug!("found modifier {attribute}");
                        modifiers.push(Modifier::attribute(attribute));
                    } else {
                        // error!(
                        //     "could not create modifier from <attribute_specifier> {}",
                        //     _Range(child.range())
                        // );
                        diags.diagnose(ReportMessage::<S::StrError>::UnknownAttributeSpecifier.at(child.range()));
                    }
                }
                "type_qualifier" => {
                    let s = match child.str(source) {
                        Ok(s) => s,
                        Err(e) => {
                            diags.diagnose(e);
                            continue;
                        }
                    };

                    if let Ok(q) = TypeQualifier::from_str(s.as_ref()) {
                        debug!("found type qualifier {q}");
                        typeQualifiers.push(q);
                    } else if let Ok(q) = DeclarationQualifier::fromAST(child, source) {
                        debug!("found decl qualifier {q}");
                        declarationQualifiers.push(q);
                    } else if let Ok(q) = FunctionQualifier::from_str(s.as_ref()) {
                        debug!("found func qualifier {q}");
                        functionQualifiers.push(q);
                    } else {
                        diags.diagnose(ReportMessage::<S::StrError>::InapplicableQualifier(s.as_ref().to_string()));
                        // warn!("qualifier '{s}' is not applicable here");
                    }
                }
                _ => continue,
            }
        } else {
            // warn!(
            //     "child {i} is neither named, nor has it a field name: {}",
            //     child.to_sexp()
            // )
        }
    }

    let Some(mut _type) = _type else {
        // error!(
        //     "declaration <{}> has no 'type' field {}",
        //     ast.kind(),
        //     _Range(ast.range())
        // );
        return Err(ReportMessage::DeclMissingType(ast.kind()).at(ast.range()));
    };

    _type.qualifiers = typeQualifiers;

    let mut msCallModifiers = Vec::<MSCallModifier>::new();

    let _declarators = declarators.as_slice();

    if _declarators.is_empty() {
        diags.diagnose(ReportMessage::<S::StrError>::MissingAnyDeclarators(ast.kind()));
        // debug!(
        //     "no declarators in <{}> {}, assuming anonymous variable -- if this is not intended, please file a bug report",
        //     ast.kind(),
        //     _Range(ast.range())
        // );

        _ = recorder.record_symbol(Symbol::variable(Variable {
            modifiers: modifiers,
            qualifiers: declarationQualifiers,
            cType: _type,
            identifier: None,
            attachedExpression: None,
        }));

        return Ok(());
    }

    for declarator in _declarators {
        let true = declarator.kind() == "ms_call_modifier" else {
            continue;
        };

        let Ok(s) = declarator.str(source) else {
            continue;
        };

        if let Ok(m) = MSCallModifier::from_str(s.as_ref()) {
            msCallModifiers.push(m)
        } else {
            // error!(
            //     "unknown MS call modifier '{s}' {}",
            //     _Range(declarator.range())
            // );
            diags.diagnose(
                ReportMessage::<S::StrError>::UnknownCallModifier(s.to_string())
                    .at(declarator.range())
            );
        }
    }

    for declarator in _declarators {
        debug!("creating symbol from <{}>", declarator.kind());
        let symbol = symbolFromDeclarator(
            declarator.clone(),
            source,
            modifiers.clone().as_mut(),
            &functionQualifiers,
            &declarationQualifiers,
            &msCallModifiers,
            _type.clone(),
            isTypeDefinition,
            &config,
            diags,
        );

        match symbol {
            Ok(symbol) => {
                debug!("symbol is: {symbol}");
                _ = recorder.record_symbol(symbol);
            }
            Err(e) => {
                // error!(
                //     "failed to create symbol from declarator: {e} {}",
                //     _Range(declarator.range())
                // );
                let l = e.location().unwrap_or(declarator.range());
                diags.diagnose(
                    ReportMessage::SemGenFailed(declarator.kind(), Box::new(e))
                        .at(l)
                );
                continue;
            }
        }
    }

    return Ok(());
}

fn symbolFromDeclarator<S: Source>(
    declarator: Node,
    source: S,
    modifiers: &mut Vec<Modifier>,
    functionQualifiers: &Vec<FunctionQualifier>,
    declarationQualifiers: &Vec<DeclarationQualifier>,
    msCallModifiers: &Vec<MSCallModifier>,
    _type: CType,
    isTypeDefinition: bool,
    config: &Config,
    diags: &impl DiagnosticReporter
) -> Result<Symbol, SemaGenError<S::StrError>> {
    let mut _type = _type;
    let mut attachedExpression: Option<AttachedExpression> = None;

    if let Some(sibling) = declarator.next_sibling() {
        if sibling.kind() == "bitfield_clause" {
            if let Some(expr) = sibling.named_child(0) {
                attachedExpression = Some(AttachedExpression::bitWidth(expr.string(source)?));
                debug!("found <bitfield_clause>");
            } else {
                // error!(
                //     "<bitfield_clause> is missing expr {}",
                //     _Range(sibling.range())
                // );
                diags.diagnose(
                        ReportMessage::<S::StrError>::MissingBitfieldClauseExpr
                            .at(sibling.range())
                    );
            }
        }
    }

    let mut _next: Option<Node> = Some(declarator);

    while let Some(current) = _next {
        match current.kind() {
            "init_declarator" => {
                let Some(next) = current.child_by_field_name("declarator") else {
                    // error!(
                    //     "<init_declarator> is missing 'declarator' field {}",
                    //     _Range(current.range())
                    // );
                    return Err(ReportMessage::MissingInnerDeclarator(current.kind()).at(current.range()));
                };

                _next = Some(next);

                if attachedExpression.is_some() {
                    // error!(
                    //     "symbol has more than one initializer {}",
                    //     _Range(current.range())
                    // );

                    if let Some(AttachedExpression::bitWidth(_)) = attachedExpression {
                        // error!(
                        //     "internal inconsistency: variable with initializer must not have bit field clause {}",
                        //     _Range(current.range())
                        // );
                        diags.diagnose(
                            ReportMessage::<S::StrError>::InitializerAndBitfieldClause
                                .at(current.range())
                        );

                    }
                    return Err(ReportMessage::DuplicateInitializer.at(current.range()));
                }
                debug!("found initial value");

                let Some(value) = current.child_by_field_name("value") else {
                    // error!(
                    //     "<init_declarator> is missing 'value' field {}",
                    //     _Range(current.range())
                    // );
                    return Err(ReportMessage::MissingValue(current.kind()).at(current.range()));
                };

                attachedExpression = Some(AttachedExpression::value(value.string(source)?));
            }
            "attributed_declarator" => {
                // attributed_declarator: $ => prec.right(seq(
                //   $._declarator,
                //   repeat1($.attribute_declaration),
                // )),

                let Some(next) = current.child(0) else {
                    // error!(
                    //     "<attributed_declarator> is missing first child {}",
                    //     _Range(current.range())
                    // );
                    return Err(ReportMessage::MissingInnerDeclarator(current.kind()).at(current.range()));
                };
                _next = Some(next);

                for attrDecl in current.children(&mut current.walk()) {
                    let true = attrDecl.kind() == "attribute_declaration" else {
                        continue;
                    };
                    // attribute_declaration: $ => seq(
                    // '[[',
                    // commaSep1($.attribute),
                    // ']]',
                    // ),

                    for attr in attrDecl.children(&mut attrDecl.walk()) {
                        let true = attr.kind() == "attribute" else {
                            continue;
                        };
                        // attribute: $ => seq(
                        //   optional(seq(field('prefix', $.identifier), '::')),
                        //   field('name', $.identifier),
                        //   optional($.argument_list),
                        // ),

                        let Some(name) = attr.child_by_field_name("name") else {
                            // error!("attribute is missing name {}", _Range(attr.range()));
                            return Err(ReportMessage::MissingStandardAttributeIdentifier.at(attr.range()));
                        };

                        let args: Vec<RawSpelling> = if let Some(list) = attr
                            .named_children(&mut attr.walk())
                            .find(|&c| c.kind() == "argument_list")
                        {
                            args_from_ast(list, source)?
                        } else {
                            Vec::new()
                        };

                        let standardAttribute = StandardAttribute {
                            prefix: match attr.child_by_field_name("prefix") {
                                Some(child) => Some(child.string(source)?),
                                None => None,
                            },
                            name: name.string(source)?,
                            arguments: args,
                        };

                        debug!("found standard attribute {standardAttribute}");

                        modifiers.push(Modifier::standardAttribute(standardAttribute));
                    }
                }
            }
            "pointer_declarator" | "abstract_pointer_declarator" => {
                // pointer_declarator: $ => prec.dynamic(1, prec.right(seq(
                //   optional($.ms_based_modifier),
                //   '*',
                //   repeat($.ms_pointer_modifier),
                //   repeat($.type_qualifier),
                //   field('declarator', $._declarator),
                // ))),

                // abstract_pointer_declarator: $ => prec.dynamic(1, prec.right(seq('*',
                //   repeat($.ms_pointer_modifier),
                //   repeat($.type_qualifier),
                //   field('declarator', optional($._abstract_declarator)),
                // ))),

                _next = current.child_by_field_name("declarator");

                let mut typeQualifiers = Vec::<TypeQualifier>::new();
                let mut pointerQualifiers = Vec::<PointerQualifier>::new();

                let mut cursor = current.walk();
                let qualifiers = current
                    .children(&mut cursor)
                    .filter(|&c| c.kind() == "type_qualifier");

                for qualifier in qualifiers {
                    let s = match qualifier.str(source) {
                        Ok(s) => s,
                        Err(e) => {
                            // error!(
                            //     "could not create string from qualifier, skipping {}",
                            //     _Range(qualifier.range())
                            // );
                            diags.diagnose(e);
                            continue;
                        }
                    };

                    if let Ok(q) = TypeQualifier::from_str(s.as_ref()) {
                        typeQualifiers.push(q);
                    } else if let Ok(q) = PointerQualifier::from_str(s.as_ref()) {
                        pointerQualifiers.push(q);
                    } else {
                        // warn!(
                        //     "qualifier '{s}' behind * is not applicable here {}",
                        //     _Range(qualifier.range())
                        // );
                        diags.diagnose(
                            ReportMessage::<S::StrError>::InapplicablePointerQualifier(s.as_ref().to_string())
                                .at(qualifier.range())
                        );
                    }
                }

                let mut msSpecificModifiers: Vec<MSPointerModifier> = current
                    .children(&mut current.walk())
                    .filter_map(|c| {
                        let true = c.kind() == "ms_pointer_modifier" else {
                            return None;
                        };

                        let Ok(name) = c.string(source) else {
                            return None;
                        };

                        return MSPointerModifier::from_ast_name(&name);
                    })
                    .collect();

                if let Some(first) = current.child(0) {
                    if first.kind() == "ms_based_modifier" {
                        let Some(args) = first.child(1) else {
                            return Err(ReportMessage::MissingArgumentList.at(first.range()));
                        };

                        assert!(args.kind() == "argument_list");

                        msSpecificModifiers
                            .push(MSPointerModifier::based(args_from_ast(args, source)?));
                    }
                }

                let decl = TypeDecl::pointer(Pointer {
                    msSpecificModifiers: msSpecificModifiers,
                    qualifiers: pointerQualifiers,
                    pointeeType: _type,
                });

                _type = CType {
                    decl: Box::new(decl),
                    qualifiers: typeQualifiers,
                };
            }
            "array_declarator" | "abstract_array_declarator" => {
                // array_declarator: $ => prec(1, seq(
                //   field('declarator', $._declarator),
                //   '[',
                //   repeat(choice($.type_qualifier, 'static')),
                //   field('size', optional(choice($.expression, '*'))),
                //   ']',
                // )),

                // abstract_array_declarator: $ => prec(1, seq(
                //   field('declarator', optional($._abstract_declarator)),
                //   '[',
                //   repeat(choice($.type_qualifier, 'static')),
                //   field('size', optional(choice($.expression, '*'))),
                //   ']',
                // )),

                _next = current.child_by_field_name("declarator");

                let countExpression: Option<Expression> = current
                    .child_by_field_name("size")
                    .map(|n| n.string(source))
                    .transpose()?;

                let mut typeQualifiers = Vec::<TypeQualifier>::new();
                let mut arrayQualifiers = Vec::<ArrayQualifier>::new();

                let mut cursor = current.walk();
                let qualifiers = current
                    .children(&mut cursor)
                    .filter(|&n| n.kind() == "type_qualifier");

                for qualifier in qualifiers {
                    let s = match qualifier.str(source) {
                        // error!(
                        //     "could create string from type qualifier, skipping {}",
                        //     _Range(qualifier.range())
                        // );
                        Ok(s) => s,
                        Err(e) => {
                            diags.diagnose(e);
                            continue;
                        }
                    };
                    if let Ok(q) = TypeQualifier::from_str(s.as_ref()) {
                        typeQualifiers.push(q);
                    } else if let Ok(q) = ArrayQualifier::from_str(s.as_ref()) {
                        arrayQualifiers.push(q);
                    } else {
                        // warn!(
                        //     "qualifier '{s}' in array braces is not applicable here {}",
                        //     _Range(qualifier.range())
                        // );
                        diags.diagnose(
                            ReportMessage::<S::StrError>::InapplicableQualifier(s.to_string())
                                .at(qualifier.range())  
                        );
                    }
                }

                let decl = TypeDecl::array(Array {
                    count: countExpression,
                    elementType: _type,
                    qualifiers: arrayQualifiers,
                });

                _type = CType {
                    decl: Box::new(decl),
                    qualifiers: typeQualifiers,
                }
            }
            "parenthesized_declarator" | "abstract_parenthesized_declarator" => {
                // parenthesized_declarator: $ => prec.dynamic(PREC.PAREN_DECLARATOR, seq(
                //   '(',
                //   optional($.ms_call_modifier),
                //   $._declarator,
                //   ')',
                // )),

                // abstract_parenthesized_declarator: $ => prec(1, seq(
                //   '(',
                //   optional($.ms_call_modifier),
                //   $._abstract_declarator,
                //   ')',
                // )),

                let endIndex = current.named_child_count();
                let true = endIndex > 0 else {
                    // error!(
                    //     "<parenthesized_declarator> is missing inner declarator {}",
                    //     _Range(current.range())
                    // );
                    return Err(ReportMessage::MissingInnerDeclarator(current.kind()).at(current.range()));
                };

                // Panic if our computed last index does not lead to a named child
                _next = Some(current.named_child(endIndex - 1).unwrap());
            }
            "function_declarator" | "abstract_function_declarator" => {
                // function_declarator: $ => prec.right(1,
                //   seq(
                //     field('declarator', $._declarator),
                //     field('parameters', $.parameter_list),
                //     optional($.gnu_asm_expression),
                //     repeat(choice(
                //       $.attribute_specifier,
                //       $.identifier,
                //       alias($.preproc_call_expression, $.call_expression),
                //     )),
                //   ),
                // ),

                // abstract_function_declarator: $ => prec(1, seq(
                //   field('declarator', optional($._abstract_declarator)),
                //   field('parameters', $.parameter_list),
                // )),

                _next = current.child_by_field_name("declarator");

                let mut parameters = Vec::<Parameter<Variable>>::new();

                if let Some(list) = current.child_by_field_name("parameters") {
                    debug!("found parameter list");

                    for p in list
                        .children(&mut list.walk())
                        .filter(|&n| n.kind() == "parameter_declaration")
                    {
                        match Parameter::from_ast(p, source, config, diags) {
                            Ok(param) =>
                                parameters.push(param),
                            Err(e) => {
                                diags.diagnose(e);
                            }
                        }
                    }
                }

                let decl = TypeDecl::function(Function {
                    modifiers: modifiers.clone(),
                    functionQualifiers: functionQualifiers.clone(),
                    declQualifiers: declarationQualifiers.clone(),
                    returnType: _type,
                    msSpecifcModifiers: msCallModifiers.clone(),
                    parameters: parameters,
                    inlineAssembly: None,
                    name: None,
                });

                _type = CType {
                    decl: Box::new(decl),
                    qualifiers: Vec::new(),
                }
            }
            "identifier" | "field_identifier" | "type_identifier" => {
                // Rust is stupid and apparently does not apply the guarding if to the pattern you attach it to.
                // Why, just why.
                // Hence, another string comparison here:
                if !isTypeDefinition && current.kind() == "type_identifier" {
                    // Great, Rust doesn't even allow me to fall through a match case.
                    // error!(
                    //     "ignoring <type_identifier>, not inside typedef -- if this is not intended please file a bug report"
                    // );
                    diags.diagnose(
                        ReportMessage::<S::StrError>::UnexpectedTypeIdentifier
                            .at(current.range())
                    );
                    continue;
                }

                // stop iteration
                _next = None;

                let identifier = current.string(source)?;

                // If this symbol is a variable with a type of function pointer, then
                // the type is .pointer(.function(...)).
                // However, if the symbol is actually a function, then we will have accidentally
                // recognized this as a variable with a type of .function.
                // This is obviously wrong. A variable can only be of type .pointer(.function(...)).
                // I.e., .function in the type can only occur in the form of .pointer(.function(...)),
                // but never alone.

                // What distinguishes a function from a variable storing a function pointer
                // is illustrated as follows. In the AST for the function pointer case, there is an
                // additional pointer_declarator.
                //
                // void f(void);
                // AST:
                // declaration
                // +- type: void
                // +- declarator:
                //    function_declarator
                //    +- ...
                //
                // void (*f)(void):
                // AST:
                // declaration
                // +- type: void
                // +- declarator:
                //    pointer_declarator
                //    +- function_declarator
                //       +- ...
                //
                // In order to avoid forward-looking logic (i.e., to check what the next, inner
                // declarator is), we naively assume the simple function case.
                if let TypeDecl::function(function) = _type.decl.as_ref() {
                    let mut f = function.clone();
                    f.name = Some(identifier);
                    return Ok(Symbol::function(f));
                } else {
                    return if isTypeDefinition {
                        Ok(Symbol::typeDefinition(TypeDefinition {
                            qualifiers: declarationQualifiers.clone(),
                            name: identifier,
                            originalType: _type,
                            attributes: modifiers
                                .iter()
                                .filter_map(|n| {
                                    if let Modifier::attribute(attribute) = n {
                                        Some(attribute.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect(),
                        }))
                    } else {
                        Ok(Symbol::variable(Variable {
                            modifiers: modifiers.clone(),
                            qualifiers: declarationQualifiers.clone(),
                            cType: _type,
                            identifier: Some(identifier),
                            attachedExpression: attachedExpression,
                        }))
                    };
                }
            }
            _ => {
                // error!(
                //     "unknown declarator <{}> {}",
                //     current.kind(),
                //     _Range(current.range())
                // );
                return Err(ReportMessage::UnknownDeclarator(current.kind()).at(current.range()));
            }
        }
    }

    if isTypeDefinition {
        // error!(
        //     "type definition is missing identifier, discarding {}",
        //     _Range(declarator.range())
        // );
        return Err(ReportMessage::MissingTypedefIdentifier.at(declarator.range()));
    }

    debug!("encountered abstract declarator");

    Ok(Symbol::variable(Variable {
        modifiers: modifiers.clone(),
        qualifiers: declarationQualifiers.clone(),
        cType: _type,
        identifier: None,
        attachedExpression: attachedExpression,
    }))
}

fn variableFieldsFromDeclList<E, S: Source, R: Recorder<E>>(
    ast: Node,
    source: S,
    config: &Config,
    _recorder: &mut R,
    diags: &impl DiagnosticReporter,
) -> Result<Vec<Variable>, SemaGenError<S::StrError>> {
    // field_declaration_list: $ => seq(
    //   '{',
    //   repeat($._field_declaration_list_item),
    //   '}',
    // ),

    let mut members = Vec::<Variable>::new();

    for child in ast.named_children(&mut ast.walk()) {
        match child.kind() {
            "field_declaration" => {
                let mut symbols = Vec::<Symbol>::new();
                match symbolsInDecl(child, source, &config, false, &mut symbols, diags) {
                    Ok(()) => {},
                    Err(e) => {
                        // error!(
                        //     "could not read symbols in field decl {}",
                        //     _Range(child.range())
                        // );
                        diags.diagnose(e);
                        continue;
                    }
                };

                for symbol in symbols {
                    let Symbol::variable(variable) = symbol else {
                        diags.diagnose(
                            ReportMessage::<S::StrError>::NonVariableMember 
                                .at(child.range())
                        );
                        // error!("field symbol is not variable");
                        continue;
                    };

                    members.push(variable)
                }
            }
            "preproc_call" => {
                diags.diagnose(
                    ReportMessage::<S::StrError>::PreprocessorCallIngored("in decl list")
                        .at(child.range())
                    );
                // warn!("encountered preprocessor call in decl list, ignoring");
            }
            "comment" => {
                // recorder.record_comment(child.slice(source)?);
            }
            _ => {
                // debug!(
                //     "ignoring <field_declaration_list> item <{}> {}",
                //     child.kind(),
                //     _Range(child.range())
                // );
                diags.diagnose(
                    ReportMessage::<S::StrError>::UnknownFieldDeclListItem(child.kind())
                        .at(child.range())
                );
            }
        }
    }

    return Ok(members);
}

fn containerFromAST<E, S: Source, R: Recorder<E>>(
    ast: Node,
    source: S,
    config: &Config,
    recorder: &mut R,
    diags: &impl DiagnosticReporter,
) -> Result<Container, SemaGenError<S::StrError>> {
    // struct_specifier: $ => prec.right(seq(
    //   'struct',
    //   optional($.attribute_specifier),
    //   optional($.ms_declspec_modifier),
    //   choice(
    //     seq(
    //       field('name', $._type_identifier),
    //       field('body', optional($.field_declaration_list)),
    //     ),
    //     field('body', $.field_declaration_list),
    //   ),
    //   optional($.attribute_specifier),
    // )),

    // union_specifier: $ => prec.right(seq(
    //   'union',
    //   optional($.ms_declspec_modifier),
    //   choice(
    //     seq(
    //       field('name', $._type_identifier),
    //       field('body', optional($.field_declaration_list)),
    //     ),
    //     field('body', $.field_declaration_list),
    //   ),
    //   optional($.attribute_specifier),
    // )),

    let mut attributes = Vec::<Attribute>::new();
    let mut msModifiers = Vec::<MSDeclModifier>::new();

    for child in ast.named_children(&mut ast.walk()) {
        match child.kind() {
            "attribute_specifier" => {
                // attribute_specifier: $ => seq(
                //   choice('__attribute__', '__attribute'),
                //   '(',
                //   $.argument_list,
                //   ')',
                // ),

                if let Some(argumentList) = child.named_child(0) {
                    let args = args_from_ast(argumentList, source)?;
                    let attribute = Attribute { arguments: args };
                    debug!("found attribute {attribute}");
                    attributes.push(attribute);
                } else {
                    // error!(
                    //     "could not create modifier from <attribute_specifier> {}",
                    //     _Range(child.range())
                    // );
                    diags.diagnose(
                        ReportMessage::<S::StrError>::UnknownAttributeSpecifier.
                            at(child.range())
                    );
                }
            }
            "ms_declspec_modifier" => {
                // ms_declspec_modifier: $ => seq(
                //   '__declspec',
                //   '(',
                //   $.identifier,
                //   ')',
                // ),

                if let Some(identifier) = child.named_child(0) {
                    match identifier.string(source) {
                        Ok(s) => {
                            msModifiers.push(MSDeclModifier::declspec(s));
                            debug!("found __declspec modifier")
                        }
                        Err(e) => {
                            // error!(
                            //     "could not create MS decl modifier from <ms_declspec_modifier>: {e} {}",
                            //     _Range(child.range())
                            // )
                            diags.diagnose(e);
                        }
                    }
                } else {
                    // error!(
                    //     "could not create MS decl modifier from <ms_declspec_modifier> {}",
                    //     _Range(child.range())
                    // )
                    diags.diagnose(
                        ReportMessage::<S::StrError>::MissingDeclSpecModifierContent
                            .at(child.range())
                    );
                }
            }
            _ => continue,
        }
    }

    let identifier: Option<String> = if let Some(identifierNode) = ast.child_by_field_name("name") {
        Some(identifierNode.string(source)?)
    } else {
        None
    };

    let body = ast.child_by_field_name("body");

    if identifier.is_none() && body.is_none() {
        // error!(
        //     "'struct' keyword used but neither identifier, nor body present {}",
        //     _Range(ast.range())
        // );
        return Err(ReportMessage::UnnamedEmptyType("struct").at(ast.range()));
    }

    let members: Vec<Variable> = if let Some(body) = body {
        variableFieldsFromDeclList(body, source, config, recorder, diags)?
    } else {
        Vec::new()
    };

    Ok(Container {
        name: identifier,
        attributes: attributes,
        msSpecificModifiers: msModifiers,
        members: members,
    })
}

fn enumCasesFromDeclList<E, S: Source, R: Recorder<E>>(
    ast: Node,
    source: S,
    _config: &Config,
    _recorder: &mut R,
    diags: &impl DiagnosticReporter
) -> Result<Vec<EnumCase>, SemaGenError<S::StrError>> {
    // enumerator_list: $ => seq(
    //   '{',
    //   repeat(choice(
    //     seq($.enumerator, ','),
    //     alias($.preproc_if_in_enumerator_list, $.preproc_if),
    //     alias($.preproc_ifdef_in_enumerator_list, $.preproc_ifdef),
    //     seq($.preproc_call, ','),
    //   )),
    //   optional(seq(
    //     choice(
    //       $.enumerator,
    //       alias($.preproc_if_in_enumerator_list_no_comma, $.preproc_if),
    //       alias($.preproc_ifdef_in_enumerator_list_no_comma, $.preproc_ifdef),
    //       $.preproc_call,
    //     ),
    //   )),
    //   '}',
    // ),

    let mut members = Vec::<EnumCase>::new();

    for child in ast.named_children(&mut ast.walk()) {
        match child.kind() {
            "enumerator" => {
                // enumerator: $ => seq(
                //   field('name', $.identifier),
                //   optional(seq('=', field('value', $.expression))),
                // ),

                let Some(nameNode) = child.child_by_field_name("name") else {
                    // error!(
                    //     "enum case is missing 'name' field, ignoring {}",
                    //     _Range(child.range())
                    // );
                    diags.diagnose(
                        ReportMessage::<S::StrError>::MissingEnumCaseName
                            .at(child.range())
                    );
                    continue;
                };

                let name = nameNode.string(source)?;

                let value: Option<Expression> =
                    if let Some(valueExpr) = child.child_by_field_name("value") {
                        Some(valueExpr.string(source)?)
                    } else {
                        None
                    };

                members.push(EnumCase { name, value });
            }
            "preproc_call" => {
                // warn!("encountered preprocessor call in decl list, ignoring");
                diags.diagnose(
                    ReportMessage::<S::StrError>::PreprocessorCallIngored("enum case decl list")
                        .at(child.range())
                );
            }
            "comment" => {
                // recorder.record_comment(child.slice(source)?);
            }
            _ => {
                debug!(
                    "ignoring <enumerator_list> item <{}> {}",
                    child.kind(),
                    _Range(child.range())
                );
                continue;
            }
        }
    }

    return Ok(members);
}

fn enumerationFromAST<E, S: Source, R: Recorder<E>>(
    ast: Node,
    source: S,
    config: &Config,
    recorder: &mut R,
    diags: &impl DiagnosticReporter
) -> Result<Enum, SemaGenError<S::StrError>> {
    // enum_specifier: $ => seq(
    //   'enum',
    //   choice(
    //     seq(
    //       field('name', $._type_identifier),
    //       optional(seq(':', field('underlying_type', $.primitive_type))),
    //       field('body', optional($.enumerator_list)),
    //     ),
    //     field('body', $.enumerator_list),
    //   ),
    //   optional($.attribute_specifier),
    // ),

    let mut attributes = Vec::<Attribute>::new();

    for child in ast.named_children(&mut ast.walk()) {
        match child.kind() {
            "attribute_specifier" => {
                // attribute_specifier: $ => seq(
                //   choice('__attribute__', '__attribute'),
                //   '(',
                //   $.argument_list,
                //   ')',
                // ),

                if let Some(argumentList) = child.named_child(0) {
                    let args = args_from_ast(argumentList, source)?;
                    let attribute = Attribute { arguments: args };
                    debug!("found attribute {attribute}");
                    attributes.push(attribute);
                } else {
                    // error!(
                    //     "could not create modifier from <attribute_specifier> {}",
                    //     _Range(child.range())
                    // );
                    diags.diagnose(
                        ReportMessage::<S::StrError>::MissingArgumentList
                            .at(child.range())
                    );
                }
            }
            _ => continue,
        }
    }

    let identifier: Option<String> = if let Some(identifierNode) = ast.child_by_field_name("name") {
        identifierNode.string(source).ok()
    } else {
        None
    };

    let mut underlyingType: Option<PrimitiveType> = None;
    if let Some(typeNode) = ast.child_by_field_name("underlying_Type") {
        let s = typeNode.str(source)?;

        if let Ok(u) = PrimitiveType::from_str(s.as_ref()) {
            underlyingType = Some(u);
        } else {
            // error!(
            //     "enum has unknown underlying type '{s}' {}",
            //     _Range(typeNode.range())
            // );
            diags.diagnose(
                ReportMessage::<S::StrError>::UnknownUnderlyingEnumType(s.as_ref().to_string())
                    .at(typeNode.range())
            );
        }
    }

    let body = ast.child_by_field_name("body");

    if identifier.is_none() && body.is_none() {
        // error!(
        //     "'enum' keyword used but neither identifier, nor body present {}",
        //     _Range(ast.range())
        // );
        return Err(ReportMessage::UnnamedEmptyType("struct").at(ast.range()));
    }

    let members: Vec<EnumCase> = if let Some(body) = body {
        enumCasesFromDeclList(body, source, config, recorder, diags)?
    } else {
        Vec::new()
    };

    return Ok(Enum {
        name: identifier,
        attributes: attributes,
        underlyingType: underlyingType,
        cases: members,
    });
}

impl CType {
    pub fn from_ast<E, S: Source, R: Recorder<E>>(
        ast: Node,
        source: S,
        qualifiers: Vec<TypeQualifier>,
        config: &Config,
        recorder: &mut R,
        diags: &impl DiagnosticReporter,
    ) -> Result<Self, SemaGenError<S::StrError>> {
        match ast.kind() {
            "struct_specifier" => {
                let container = containerFromAST(ast, source, config, recorder, diags)?;

                Ok(Self {
                    decl: Box::new(TypeDecl::structure(container)),
                    qualifiers: qualifiers,
                })
            }
            "union_specifier" => {
                let container = containerFromAST(ast, source, config, recorder, diags)?;

                Ok(Self {
                    decl: Box::new(TypeDecl::union(container)),
                    qualifiers: qualifiers,
                })
            }
            "enum_specifier" => {
                let _enum = enumerationFromAST(ast, source, config, recorder, diags)?;

                Ok(Self {
                    decl: Box::new(TypeDecl::enumeration(_enum)),
                    qualifiers: qualifiers,
                })
            }
            "primitive_type" => {
                let s = ast.str(source)?;

                let Ok(primitiveType) = PrimitiveType::from_str(s.as_ref()) else {
                    // error!("Unknown primitive type");
                    return Err(ReportMessage::UnknownPrimitiveType(s.to_string()).at(ast.range()));
                };

                let decl = TypeDecl::named(NamedType::primitive(primitiveType), Vec::new());
                Ok(Self {
                    decl: Box::new(decl),
                    qualifiers: qualifiers,
                })
            }
            "identifier" | "type_identifier" => {
                let s = ast.string(source)?;
                let decl = TypeDecl::named(NamedType::custom(s), Vec::new());
                Ok(Self {
                    decl: Box::new(decl),
                    qualifiers: qualifiers,
                })
            }
            "sized_type_specifier" => {
                let mut _type = if let Some(typeAST) = ast.child_by_field_name("type") {
                    CType::from_ast(typeAST, source, qualifiers, config, recorder, diags)?
                } else {
                    CType {
                        decl: Box::new(TypeDecl::named(
                            NamedType::primitive(PrimitiveType::int),
                            Vec::new(),
                        )),
                        qualifiers: Vec::new(),
                    }
                };

                let mut iter = ast.walk();
                for child in ast.children(&mut iter) {
                    if child.is_named() {
                        match child.kind() {
                            "type_qualifier" => {
                                let Ok(s) = child.str(source) else {
                                    continue;
                                };

                                if let Ok(q) = TypeQualifier::from_str(s.as_ref()) {
                                    debug!("found type qualifier {q}");
                                    _type.qualifiers.push(q);
                                } else {
                                    // warn!("qualifier '{s}' is applicable here");
                                    diags.diagnose(
                                        ReportMessage::<S::StrError>::InapplicablePointerQualifier(s.to_string())
                                            .at(child.range())
                                    );
                                }
                            }
                            _ => {}
                        }
                    } else {
                        let Ok(s) = child.str(source) else {
                            continue;
                        };
                        if let Ok(q) = SizeModifier::from_str(s.as_ref()) {
                            debug!("found size modifier {q}");
                            let TypeDecl::named(_, modifiers) = _type.decl.as_mut() else {
                                continue;
                            };

                            modifiers.push(q);
                        } else {
                            // error!(
                            //     "internal inconsistency: unexpected qualifier '{s}' in <sized_type_specifier>"
                            // );
                            diags.diagnose(
                                ReportMessage::<S::StrError>::UnexpectedQualifier(
                                    s.to_string(), child.kind()
                                ).at(child.range())
                            );
                        }
                    }
                }

                Ok(_type)
            }
            _ => {
                // error!(
                //     "unknown C declarator type (leaf type): <{}> '{}' {}",
                //     ast.kind(),
                //     ast.string(source).unwrap_or("op".to_string()),
                //     _Range(ast.range())
                // );
                Err(ReportMessage::UnknownDeclarator(ast.kind()).at(ast.range()))
            }
        }
    }
}

impl DeclarationQualifier {
    fn fromAST<S: Source>(ast: Node, source: S) -> Result<Self, SemaGenError<S::StrError>> {
        let Some(child) = ast.child(0) else {
            // error!("<type_qualifier> is missing child {}", _Range(ast.range()));
            return Err(ReportMessage::MissingTypeQualifierChild.at(ast.range()));
        };

        if child.is_named() && child.kind() == "alignas_qualifier" {
            // TODO: handle alignas_qualifier
            Ok(Self::alignas("".to_string()))
        } else {
            let s = ast.str(source)?;
            match s.as_ref() {
                "constexpr" => Ok(Self::constexpr),
                "__extension__" => Ok(Self::extension),
                _ => {
                    // error!("unknown decl qualifier: '{}' {}", s.as_ref(), _Range(child.range()));
                    Err(ReportMessage::UnknownDeclQualifier(s.to_string()).at(child.range()))
                }
            }
        }
    }
}

impl MSPointerModifier {
    fn from_ast_name(astName: &str) -> Option<Self> {
        match astName {
            "ms_unaligned_ptr_modifier" => Some(Self::unaligned),
            "ms_restrict_modifier" => Some(Self::restrict),
            "ms_unsigned_ptr_modifier" => Some(Self::unsigned),
            "ms_signed_ptr_modifier" => Some(Self::signed),
            _ => None,
        }
    }
}

impl Parameter<Variable> {
    fn from_ast<S: Source>(
        ast: Node,
        source: S,
        config: &Config,
        diags: &impl DiagnosticReporter,
    ) -> Result<Self, SemaGenError<S::StrError>> {
        match ast.kind() {
            "parameter_declaration" => {
                // parameter_declaration: $ => seq(
                //   $._declaration_specifiers,
                //   optional(field('declarator', choice(
                //     $._declarator,
                //     $._abstract_declarator,
                //   ))),
                //   repeat($.attribute_specifier),
                // ),

                let mut symbols = Vec::<Symbol>::new();
                symbolsInDecl(ast, source, &config, false, &mut symbols, diags)?;

                if symbols.len() != 1 {
                    return Err(ReportMessage::WeirdFunctionParameter.at(ast.range()));
                }

                let Symbol::variable(variable) = symbols.first().unwrap() else {
                    return Err(ReportMessage::WeirdFunctionParameter.at(ast.range()));
                };
                Ok(Self::regular(variable.clone()))
            }
            "variadic_parameter" => Ok(Self::variadic(None)),
            _ => {
                // error!(
                //     "cannot create parameter from <{}> {}",
                //     ast.kind(),
                //     _Range(ast.range())
                // );
                Err(ReportMessage::UnsupportedFunctionParameterDecl(ast.kind()).at(ast.range()))
            }
        }
    }
}
