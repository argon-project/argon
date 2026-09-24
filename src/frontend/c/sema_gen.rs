use super::sema::*;
pub use super::recorder::*;
use crate::{
    compiler::{
        diagnostics::{
            self, Diagnostic, DiagnosticReporter, LocatedDiagnostic, LocationAttachableDiagnostic
        }, strings::Source
    }
};

use std::{collections::HashMap, marker::PhantomData};
use std::str::FromStr;
use std::{error, fmt};
use log::{
    debug, 
    // error,
    // warn
};
use tree_sitter::{
    Node,
};

const ENCLOSED_COMMENT_NODE_NAME_PREFIX: &'static str = "comment:";

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

#[derive(Debug, Clone)]
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

pub struct ASTWalker<'a, S: Source, R: CLanguageRecorder<E>, E, D: DiagnosticReporter> {
    source: S,
    config: &'a Config,
    recorder: R,
    diags: &'a D,
    _bla: PhantomData<E>,
}

impl<'a, S: Source, R: CLanguageRecorder<E>, E, D: DiagnosticReporter> ASTWalker<'a, S, R, E, D> {
    pub fn new(source: S, config: &'a Config, recorder: R, diags: &'a D) -> Self {
        Self { source, config, recorder, diags, _bla: PhantomData }
    }

    fn with_recorder<R2: CLanguageRecorder<E2>, E2>(&self, other_recorder: R2) -> ASTWalker<'a, S, R2, E2, D> {
        ASTWalker::new(self.source, self.config, other_recorder, self.diags)
    }

    fn args_from_ast(
        &mut self,
        argument_list: Node,
    ) -> Result<Vec<RawSpelling>, SemaGenError<S::StrError>> {
        argument_list
            .named_children(&mut argument_list.walk())
            .map(|s| s.string(self.source))
            .collect()
    }

    fn evaluatePreprocessorExpression(
        &mut self,
        ast: Node,
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
                let key = ast.str(self.source)?;

                Ok(self.config
                    .preprocessor_defines
                    .get(key.as_ref())
                    .map_or_else(|| 0, |&v| v))
            }
            "call_expression" => {
                debug!("preprocessor call expr not supported, evaluating to false");
                Ok(0)
            }
            "number_literal" => isize::from_str(&ast.str(self.source)?.as_ref())
                .map_err(|e| ReportMessage::InvalidNumberLiteral(e).at(ast.range())),
            "char_literal" => {
                let mut value: isize = 0;
                let mut iter = ast.walk();
                for char in ast.named_children(&mut iter) {
                    let c = char::from_str(&char.str(self.source)?.as_ref())
                        .map_err(|e| ReportMessage::InvalidCharacterLiteral(e).at(ast.range()))?;
                    value += c as u32 as isize;
                }

                Ok(value)
            }
            "preproc_defined" => {
                let Some(identifier) = ast.named_child(0) else {
                    return Err(ReportMessage::PreprocessorMissingIdentifier.at(ast.range()));
                };

                Ok(self.config
                    .preprocessor_defines
                    .contains_key(identifier.str(self.source)?.as_ref())
                    .into())
            }
            "unary_expression" => {
                let operator = ast
                    .child_by_field_name("operator")
                    .ok_or(ReportMessage::MissingOperator.at(ast.range()))?;

                let operand = ast
                    .child_by_field_name("argument")
                    .ok_or(ReportMessage::MissingOperand.at(ast.range()))?;

                let op = operator.str(self.source)?;
                let operand = self.evaluatePreprocessorExpression(operand)?;

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

                let _op = operator.str(self.source)?;
                let op = _op.as_ref();

                let left = self.evaluatePreprocessorExpression(left)?;
                let right = self.evaluatePreprocessorExpression(right)?;

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
            "parenthesized_expression" => self.evaluatePreprocessorExpression(
                ast.named_child(0)
                    .ok_or_else(|| ReportMessage::MissingOperand.at(ast.range()))?,
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

    fn symbolizePreprocessorConditionalBranch(
        &mut self,
        ast: Node,
    ) -> Result<(), SemaGenError<S::StrError>> {
        let mut iter = ast.walk();
        for child in ast.named_children(&mut iter).skip(1) {
            match child.kind() {
                "preproc_elif" | "preproc_else" => continue,
                _ => {}
            }

            match self.gen_sema(child) {
                Err(e) => return Err(e),
                _ => {}
            }
        }

        Ok(())
    }

    fn symbolizePreprocessorConditionalContent(
        &mut self,
        ast: Node,
    ) -> Result<(), SemaGenError<S::StrError>> {
        let Some(condition) = ast.child_by_field_name("condition") else {
            // error!(
            //     "internal inconsistency: <preproc_if> is missing 'condition' field {}",
            //     _Range(ast.range())
            // );
            return Err(ReportMessage::MissingCondition.at(ast.range()));
        };

        if self.evaluatePreprocessorExpression(condition)? != 0 {
            self.symbolizePreprocessorConditionalBranch(ast)
        } else {
            let Some(alternative) = ast.child_by_field_name("alternative") else {
                debug!("conditional preprocessor content has no alternative");
                return Ok(());
            };

            match alternative.kind() {
                "preproc_elif" => {
                    self.symbolizePreprocessorConditionalContent(alternative)
                }
                "preproc_else" => self.symbolizePreprocessorConditionalBranch(ast),
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

    pub fn symbolizePreprocessorIfDefinedConditionalContent(
        &mut self,
        ast: Node,
    ) -> Result<(), SemaGenError<S::StrError>> {
        let Some(directive) = ast.child(0) else {
            // error!(
            //     "preprocessor invocation is missing directive {}",
            //     _Range(ast.range())
            // );
            return Err(ReportMessage::MissingDirective.at(ast.range()));
        };

        let _s = directive.str(self.source)?;
        let s = _s.as_ref();
        let Some(identifier_node) = ast.child_by_field_name("name") else {
            // error!(
            //     "internal inconsistency: preproc_ifdef is missing 'name' field {}",
            //     _Range(ast.range())
            // );
            return Err(ReportMessage::PreprocessorMissingIdentifier.at(directive.range()));
        };

        let identifier = identifier_node.str(self.source)?;

        let mut conditionMet = self.config
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
            self.diags.diagnose(
                ReportMessage::<S::StrError>::UnknownConditionalPreprocessorDirective(s.to_string())
                    .at(identifier_node.range())
            );
        }

        if conditionMet {
            self.gen_sema(ast)
        } else {
            debug!(
                "ignoring conditional preprocessor directive '{s} {} {}', condition not met",
                identifier.as_ref(),
                _Range(directive.range())
            );

            self.diags.diagnose(
                ReportMessage::<S::StrError>::UnsatisfiedPreprocessorCondition((s.to_string() + " ") + identifier.as_ref())
                    .at(directive.range())
            );

            Ok(())
        }
    }

    pub fn gen_sema(
        &mut self,
        ast: Node,
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
                self.diags.diagnose(
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
                        self.diags.diagnose(
                            ReportMessage::<S::StrError>::PreprocessorMissingIncludePath
                                .at(child.range())
                            );
                        continue;
                    };

                    match path.kind() {
                        "system_lib_string" => {
                            let _s = path.str(self.source)?;
                            let s = _s.as_ref();
                            _ = self.recorder.record_include(IncludedHeader::system(s[1..s.len() - 1].into()));
                        }
                        "string_literal" => {
                            let Some(s) = path.child(1) else {
                                // error!("#include is missing path {}", _Range(path.range()));
                                self.diags.diagnose(
                                    ReportMessage::<S::StrError>::PreprocessorMissingIncludePathLocal
                                        .at(path.range())
                                    );
                                continue;
                            };

                            _ = self.recorder.record_include(IncludedHeader::local(s.string(self.source)?));
                        }
                        _ => {}
                    }
                }
                "preproc_if" => {
                    match self.symbolizePreprocessorConditionalContent(child) {
                        Err(e) => 
                            self.diags.diagnose(
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
                    match self.symbolizePreprocessorIfDefinedConditionalContent(
                        child
                    ) {
                        Err(e) => self.diags.diagnose(
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
                    match self.symbolizeMacroDefintion(child) {
                        Err(e) => {
                           self.diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                        },
                        _ => {}
                    }
                }
                "declaration" | "function_definition" => {
                    debug!("encountered decl");
                    match self.visit_decl(child, false) {
                        Err(e) => {
                            self.diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                        },
                        _ => {}
                    }
                }
                "struct_specifier" => {
                    match self.containerFromAST(child) {
                        Ok((_struct, body)) => {
                            // If this fails, we cannot do anything..
                            _ = self.recorder.record_symbol(Symbol::structure(_struct), child.range());
                            if let Some(body) = body {
                                match self.visit_field_decl_list(body) {
                                    _ => {}
                                    Err(e) => {
                                        self.diags.diagnose(ReportMessage::SemGenFailed(body.kind(), Box::new(e)));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            self.diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                        },
                    }
                }
                "union_specifier" => {
                    match self.containerFromAST(child) {
                        Ok((_union, body)) => {
                            // If this fails, we cannot do anything..
                            _ = self.recorder.record_symbol(Symbol::union(_union), child.range());
                            if let Some(body) = body {
                                match self.visit_field_decl_list(body) {
                                    _ => {}
                                    Err(e) => {
                                        self.diags.diagnose(ReportMessage::SemGenFailed(body.kind(), Box::new(e)));
                                    }
                                }
                            }
                        }
                        Err(e) =>  {
                            self.diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                        },
                    }
                }
                "enum_specifier" => {
                    match self.enumerationFromAST(child) {
                        Ok((_enum, body)) => {
                            // If this fails, we cannot do anything..
                            _ = self.recorder.record_symbol(Symbol::enumeration(_enum), child.range());
                            if let Some(body) = body {
                                match self.visit_enum_decl_list(body) {
                                    _ => {}
                                    Err(e) => {
                                        self.diags.diagnose(ReportMessage::SemGenFailed(body.kind(), Box::new(e)));
                                    }
                                }
                            }
                        }
                        Err(e) =>  {
                            self.diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                        },
                    }
                }
                "type_definition" => {
                    debug!("encountered typedef");
                    match self.visit_decl(child, true) {
                        Err(e) =>  {
                            self.diags.diagnose(ReportMessage::SemGenFailed(child.kind(), Box::new(e)));
                        },
                        _ => {}
                    }
                }
                "comment" => {
                    // normal comment? nah.
                }
                other => {
                    if let Some(style) = other.strip_prefix(ENCLOSED_COMMENT_NODE_NAME_PREFIX) {
                        self.recorder.record_comment(child.str(self.source)?.as_ref(), style, child.range());
                    } else {
                        debug!(
                            "ignoring scope child <{}> {}",
                            child.kind(),
                            _Range(child.range())
                        );
                    }
                }
            }
        }

        Ok(())
    }

    pub fn symbolizeMacroDefintion(
        &mut self,
        ast: Node,
    ) -> Result<(), SemaGenError<S::StrError>> {
        let name = ast
            .child_by_field_name("name")
            .ok_or(ReportMessage::PreprocessorMissingIdentifier.at(ast.range()))?
            .str(self.source)?;

        if self.config.ingore_header_guard_defines && name.as_ref().ends_with("_H") {
            debug!("ignoring header guard definition");
            return Ok(());
        }

        let mut parameters = Vec::<Parameter<Identifier>>::new();

        if let Some(params) = ast.child_by_field_name("parameters") {
            let mut iter = params.walk();
            for child in params.children(&mut iter) {
                let s = child.str(self.source)?;
                if child.is_named() {
                    let true = child.kind() == "identifier" else {
                        // error!(
                        //     "<preproc_params has unknown child <{}> {}",
                        //     child.kind(),
                        //     _Range(child.range())
                        // );
                        self.diags.diagnose(
                            ReportMessage::<S::StrError>::UnknownChildNode(ast.kind(), child.kind())
                                .at(child.range())
                        );
                        continue;
                    };

                    parameters.push(Parameter::regular(s.to_string()));
                } else {
                    let true = s.as_ref() == "..." else {
                        // error!(
                        //     "<preproc_params has unknown child '{}' {}",
                        //     child.kind(),
                        //     _Range(child.range())
                        // );
                        self.diags.diagnose(
                            ReportMessage::<S::StrError>::UnknownChildNode(ast.kind(), child.kind())
                                .at(child.range())
                        );
                        continue;
                    };

                    parameters.push(Parameter::variadic(None));
                }
            }
        }

        _ = self.recorder.record_symbol(Symbol::macroDefintion(MacroDefinition {
            name: name.to_string(),
            parameters: parameters,
            value: ast
                .child_by_field_name("value")
                .map(|v| v.string(self.source))
                .transpose()?,
        }), ast.range());

        Ok(())
    }

    pub fn visit_decl(
        &mut self,
        ast: Node,
        isTypeDefinition: bool,
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
                if let Ok(name) = first.str(self.source) {
                    if name.as_ref() == "__extension__" {
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
                        _type = Some(self.type_from_ast(
                            child,
                            Vec::new(),
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
                        let Ok(s) = child.str(self.source) else {
                            continue;
                        };

                        let Ok(storage) = Storage::from_str(s.as_ref()) else {
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
                            let args= self.args_from_ast(argumentList)?;
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
                            self.diags.diagnose(ReportMessage::<S::StrError>::UnknownAttributeSpecifier.at(child.range()));
                        }
                    }
                    "type_qualifier" => {
                        let s = match child.str(self.source) {
                            Ok(s) => s,
                            Err(e) => {
                                self.diags.diagnose(e);
                                continue;
                            }
                        };

                        if let Ok(q) = TypeQualifier::from_str(s.as_ref()) {
                            debug!("found type qualifier {q}");
                            typeQualifiers.push(q);
                        } else if let Ok(q) = self.declaration_fromAST(child) {
                            debug!("found decl qualifier {q}");
                            declarationQualifiers.push(q);
                        } else if let Ok(q) = FunctionQualifier::from_str(s.as_ref()) {
                            debug!("found func qualifier {q}");
                            functionQualifiers.push(q);
                        } else {
                            self.diags.diagnose(ReportMessage::<S::StrError>::InapplicableQualifier(s.as_ref().to_string()));
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
            self.diags.diagnose(ReportMessage::<S::StrError>::MissingAnyDeclarators(ast.kind()));
            // debug!(
            //     "no declarators in <{}> {}, assuming anonymous variable -- if this is not intended, please file a bug report",
            //     ast.kind(),
            //     _Range(ast.range())
            // );

            _ = self.recorder.record_symbol(Symbol::variable(Variable {
                modifiers: modifiers,
                qualifiers: declarationQualifiers,
                cType: _type,
                identifier: None,
                attachedExpression: None,
            }), ast.range());

            return Ok(());
        }

        for declarator in _declarators {
            let true = declarator.kind() == "ms_call_modifier" else {
                continue;
            };

            let Ok(s) = declarator.str(self.source) else {
                continue;
            };

            if let Ok(m) = MSCallModifier::from_str(s.as_ref()) {
                msCallModifiers.push(m)
            } else {
                // error!(
                //     "unknown MS call modifier '{s}' {}",
                //     _Range(declarator.range())
                // );
                self.diags.diagnose(
                    ReportMessage::<S::StrError>::UnknownCallModifier(s.to_string())
                        .at(declarator.range())
                );
            }
        }

        for declarator in _declarators {
            debug!("creating symbol from <{}>", declarator.kind());
            let symbol = self.symbolFromDeclarator(
                declarator.clone(),
                modifiers.clone().as_mut(),
                &functionQualifiers,
                &declarationQualifiers,
                &msCallModifiers,
                _type.clone(),
                isTypeDefinition,
            );

            match symbol {
                Ok(symbol) => {
                    debug!("symbol is: {symbol}");
                    _ = self.recorder.record_symbol(symbol, ast.range());
                }
                Err(e) => {
                    // error!(
                    //     "failed to create symbol from declarator: {e} {}",
                    //     _Range(declarator.range())
                    // );
                    let l = e.location().unwrap_or(declarator.range());
                    self.diags.diagnose(
                        ReportMessage::SemGenFailed(declarator.kind(), Box::new(e))
                            .at(l)
                    );
                    continue;
                }
            }
        }

        return Ok(());
    }

    fn symbolFromDeclarator(
        &mut self,
        declarator: Node,
        modifiers: &mut Vec<Modifier>,
        functionQualifiers: &Vec<FunctionQualifier>,
        declarationQualifiers: &Vec<DeclarationQualifier>,
        msCallModifiers: &Vec<MSCallModifier>,
        _type: CType,
        isTypeDefinition: bool,
    ) -> Result<Symbol, SemaGenError<S::StrError>> {
        let mut _type = _type;
        let mut attachedExpression: Option<AttachedExpression> = None;

        if let Some(sibling) = declarator.next_sibling() {
            if sibling.kind() == "bitfield_clause" {
                if let Some(expr) = sibling.named_child(0) {
                    attachedExpression = Some(AttachedExpression::bitWidth(expr.string(self.source)?));
                    debug!("found <bitfield_clause>");
                } else {
                    // error!(
                    //     "<bitfield_clause> is missing expr {}",
                    //     _Range(sibling.range())
                    // );
                    self.diags.diagnose(
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
                            self.diags.diagnose(
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

                    attachedExpression = Some(AttachedExpression::value(value.string(self.source)?));
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
                                self.args_from_ast(list)?
                            } else {
                                Vec::new()
                            };

                            let standardAttribute = StandardAttribute {
                                prefix: match attr.child_by_field_name("prefix") {
                                    Some(child) => Some(child.string(self.source)?),
                                    None => None,
                                },
                                name: name.string(self.source)?,
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
                        let s = match qualifier.str(self.source) {
                            Ok(s) => s,
                            Err(e) => {
                                // error!(
                                //     "could not create string from qualifier, skipping {}",
                                //     _Range(qualifier.range())
                                // );
                                self.diags.diagnose(e);
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
                            self.diags.diagnose(
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

                            let Ok(name) = c.str(self.source) else {
                                return None;
                            };

                            return MSPointerModifier::from_ast_name(name.as_ref());
                        })
                        .collect();

                    if let Some(first) = current.child(0) {
                        if first.kind() == "ms_based_modifier" {
                            let Some(args) = first.child(1) else {
                                return Err(ReportMessage::MissingArgumentList.at(first.range()));
                            };

                            assert!(args.kind() == "argument_list");

                            msSpecificModifiers
                                .push(MSPointerModifier::based(self.args_from_ast(args)?));
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
                        .map(|n| n.string(self.source))
                        .transpose()?;

                    let mut typeQualifiers = Vec::<TypeQualifier>::new();
                    let mut arrayQualifiers = Vec::<ArrayQualifier>::new();

                    let mut cursor = current.walk();
                    let qualifiers = current
                        .children(&mut cursor)
                        .filter(|&n| n.kind() == "type_qualifier");

                    for qualifier in qualifiers {
                        let s = match qualifier.str(self.source) {
                            // error!(
                            //     "could create string from type qualifier, skipping {}",
                            //     _Range(qualifier.range())
                            // );
                            Ok(s) => s,
                            Err(e) => {
                                self.diags.diagnose(e);
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
                            self.diags.diagnose(
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
                            match self.parameter_from_ast(p) {
                                Ok(param) =>
                                    parameters.push(param),
                                Err(e) => {
                                    self.diags.diagnose(e);
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
                        self.diags.diagnose(
                            ReportMessage::<S::StrError>::UnexpectedTypeIdentifier
                                .at(current.range())
                        );
                        continue;
                    }

                    // stop iteration
                    _next = None;

                    let identifier = current.string(self.source)?;

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

    fn visit_field_decl_list(
        &mut self,
        ast: Node,
    ) -> Result<(), SemaGenError<S::StrError>> {
        // field_declaration_list: $ => seq(
        //   '{',
        //   repeat($._field_declaration_list_item),
        //   '}',
        // ),
        
        for child in ast.named_children(&mut ast.walk()) {
            match child.kind() {
                "field_declaration" => {
                    match self.visit_decl(child, false) {
                        Ok(()) => {},
                        Err(e) => {
                            // error!(
                            //     "could not read symbols in field decl {}",
                            //     _Range(child.range())
                            // );
                            self.diags.diagnose(e);
                            continue;
                        }
                    };
                }
                "preproc_call" => {
                    self.diags.diagnose(
                        ReportMessage::<S::StrError>::PreprocessorCallIngored("in decl list")
                            .at(child.range())
                        );
                    // warn!("encountered preprocessor call in decl list, ignoring");
                }
                "comment" => {
                    // normal comment? nah.
                }
                other => {
                    if let Some(style) = other.strip_prefix(ENCLOSED_COMMENT_NODE_NAME_PREFIX) {
                        self.recorder.record_comment(child.str(self.source)?.as_ref(), style, child.range());
                    } else {
                        debug!(
                            "ignoring <field_declaration_list> item <{}> {}",
                            child.kind(),
                            _Range(child.range())
                        );
                        
                        self.diags.diagnose(
                            ReportMessage::<S::StrError>::UnknownFieldDeclListItem(child.kind())
                                .at(child.range())
                        );
                    }
                }
            }
        }

        return Ok(());
    }

    fn containerFromAST<'b>(
        &mut self,
        ast: Node<'b>,
    ) -> Result<(Container, Option<Node<'b>>), SemaGenError<S::StrError>> {
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
                        let args = self.args_from_ast(argumentList)?;
                        let attribute = Attribute { arguments: args };
                        debug!("found attribute {attribute}");
                        attributes.push(attribute);
                    } else {
                        // error!(
                        //     "could not create modifier from <attribute_specifier> {}",
                        //     _Range(child.range())
                        // );
                        self.diags.diagnose(
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
                        match identifier.string(self.source) {
                            Ok(s) => {
                                msModifiers.push(MSDeclModifier::declspec(s));
                                debug!("found __declspec modifier")
                            }
                            Err(e) => {
                                // error!(
                                //     "could not create MS decl modifier from <ms_declspec_modifier>: {e} {}",
                                //     _Range(child.range())
                                // )
                                self.diags.diagnose(e);
                            }
                        }
                    } else {
                        // error!(
                        //     "could not create MS decl modifier from <ms_declspec_modifier> {}",
                        //     _Range(child.range())
                        // )
                        self.diags.diagnose(
                            ReportMessage::<S::StrError>::MissingDeclSpecModifierContent
                                .at(child.range())
                        );
                    }
                }
                _ => continue,
            }
        }

        let identifier: Option<String> = if let Some(identifierNode) = ast.child_by_field_name("name") {
            Some(identifierNode.string(self.source)?)
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

        Ok((Container {
            name: identifier,
            attributes: attributes,
            msSpecificModifiers: msModifiers,
            members: Vec::new(),
        }, body))
    }

    fn visit_enum_decl_list(
        &mut self,
        ast: Node,
    ) -> Result<(), SemaGenError<S::StrError>> {
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
                        self.diags.diagnose(
                            ReportMessage::<S::StrError>::MissingEnumCaseName
                                .at(child.range())
                        );
                        continue;
                    };

                    let name = nameNode.string(self.source)?;

                    let value: Option<Expression> =
                        if let Some(valueExpr) = child.child_by_field_name("value") {
                            Some(valueExpr.string(self.source)?)
                        } else {
                            None
                        };
                    self.recorder.record_symbol(Symbol::enumerationCase(EnumCase { name, value }), child.range());
                }
                "preproc_call" => {
                    // warn!("encountered preprocessor call in decl list, ignoring");
                    self.diags.diagnose(
                        ReportMessage::<S::StrError>::PreprocessorCallIngored("enum case decl list")
                            .at(child.range())
                    );
                }
                "comment" => {
                    // normal comment? nah.
                }
                other => {
                    if let Some(style) = other.strip_prefix(ENCLOSED_COMMENT_NODE_NAME_PREFIX) {
                        self.recorder.record_comment(child.str(self.source)?.as_ref(), style, child.range());
                    } else {
                        debug!(
                            "ignoring <enumerator_list> item <{}> {}",
                            child.kind(),
                            _Range(child.range())
                        );
                    }
                    continue;
                }
            }
        }

        Ok(())
    }

    fn enumerationFromAST<'b>(
        &mut self,
        ast: Node<'b>,
    ) -> Result<(Enum, Option<Node<'b>>), SemaGenError<S::StrError>> {
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
                        let args = self.args_from_ast(argumentList)?;
                        let attribute = Attribute { arguments: args };
                        debug!("found attribute {attribute}");
                        attributes.push(attribute);
                    } else {
                        // error!(
                        //     "could not create modifier from <attribute_specifier> {}",
                        //     _Range(child.range())
                        // );
                        self.diags.diagnose(
                            ReportMessage::<S::StrError>::MissingArgumentList
                                .at(child.range())
                        );
                    }
                }
                _ => continue,
            }
        }

        let identifier: Option<String> = if let Some(identifierNode) = ast.child_by_field_name("name") {
            identifierNode.string(self.source).ok()
        } else {
            None
        };

        let mut underlyingType: Option<PrimitiveType> = None;
        if let Some(typeNode) = ast.child_by_field_name("underlying_Type") {
            let s = typeNode.str(self.source)?;

            if let Ok(u) = PrimitiveType::from_str(s.as_ref()) {
                underlyingType = Some(u);
            } else {
                // error!(
                //     "enum has unknown underlying type '{s}' {}",
                //     _Range(typeNode.range())
                // );
                self.diags.diagnose(
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

        return Ok((Enum {
            name: identifier,
            attributes: attributes,
            underlyingType: underlyingType,
            cases: Vec::new(),
        }, body));
    }

    pub fn type_from_ast(
        &mut self,
        ast: Node,
        qualifiers: Vec<TypeQualifier>,
    ) -> Result<CType, SemaGenError<S::StrError>> {
        match ast.kind() {
            "struct_specifier" => {
                let (mut container, body) = self.containerFromAST(ast)?;
                if let Some(body) = body {
                    self.with_recorder(&mut container.members)
                        .visit_field_decl_list(body)?;
                }

                Ok(CType {
                    decl: Box::new(TypeDecl::structure(container)),
                    qualifiers: qualifiers,
                })
            }
            "union_specifier" => {
                let (mut container, body) = self.containerFromAST(ast)?;
                if let Some(body) = body {
                    self.with_recorder(&mut container.members)
                        .visit_field_decl_list(body)?;
                }

                Ok(CType {
                    decl: Box::new(TypeDecl::union(container)),
                    qualifiers: qualifiers,
                })
            }
            "enum_specifier" => {
                let (mut _enum, body) = self.enumerationFromAST(ast)?;
                if let Some(body) = body {
                    self.with_recorder(&mut _enum.cases)
                        .visit_enum_decl_list(body)?;
                }

                Ok(CType {
                    decl: Box::new(TypeDecl::enumeration(_enum)),
                    qualifiers: qualifiers,
                })
            }
            "primitive_type" => {
                let s = ast.str(self.source)?;

                let Ok(primitiveType) = PrimitiveType::from_str(s.as_ref()) else {
                    // error!("Unknown primitive type");
                    return Err(ReportMessage::UnknownPrimitiveType(s.to_string()).at(ast.range()));
                };

                let decl = TypeDecl::named(NamedType::primitive(primitiveType), Vec::new());
                Ok(CType {
                    decl: Box::new(decl),
                    qualifiers: qualifiers,
                })
            }
            "identifier" | "type_identifier" => {
                let s = ast.string(self.source)?;
                let decl = TypeDecl::named(NamedType::custom(s), Vec::new());
                Ok(CType {
                    decl: Box::new(decl),
                    qualifiers: qualifiers,
                })
            }
            "sized_type_specifier" => {
                let mut _type = if let Some(typeAST) = ast.child_by_field_name("type") {
                    self.type_from_ast(typeAST, qualifiers)?
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
                                let Ok(s) = child.str(self.source) else {
                                    continue;
                                };

                                if let Ok(q) = TypeQualifier::from_str(s.as_ref()) {
                                    debug!("found type qualifier {q}");
                                    _type.qualifiers.push(q);
                                } else {
                                    // warn!("qualifier '{s}' is applicable here");
                                    self.diags.diagnose(
                                        ReportMessage::<S::StrError>::InapplicablePointerQualifier(s.to_string())
                                            .at(child.range())
                                    );
                                }
                            }
                            _ => {}
                        }
                    } else {
                        let Ok(s) = child.str(self.source) else {
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
                            self.diags.diagnose(
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

    
    fn declaration_fromAST(&mut self, ast: Node) -> Result<DeclarationQualifier, SemaGenError<S::StrError>> {
        let Some(child) = ast.child(0) else {
            // error!("<type_qualifier> is missing child {}", _Range(ast.range()));
            return Err(ReportMessage::MissingTypeQualifierChild.at(ast.range()));
        };

        if child.is_named() && child.kind() == "alignas_qualifier" {
            // TODO: handle alignas_qualifier
            Ok(DeclarationQualifier::alignas("".to_string()))
        } else {
            let s = ast.str(self.source)?;
            match s.as_ref() {
                "constexpr" => Ok(DeclarationQualifier::constexpr),
                "__extension__" => Ok(DeclarationQualifier::extension),
                _ => {
                    // error!("unknown decl qualifier: '{}' {}", s.as_ref(), _Range(child.range()));
                    Err(ReportMessage::UnknownDeclQualifier(s.to_string()).at(child.range()))
                }
            }
        }
    }

    fn parameter_from_ast(
        &mut self,
        ast: Node,
    ) -> Result<Parameter<Variable>, SemaGenError<S::StrError>> {
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
                self.with_recorder(&mut symbols).visit_decl(ast, false)?;

                if symbols.len() != 1 {
                    return Err(ReportMessage::WeirdFunctionParameter.at(ast.range()));
                }

                let Symbol::variable(variable) = symbols.first().unwrap() else {
                    return Err(ReportMessage::WeirdFunctionParameter.at(ast.range()));
                };
                Ok(Parameter::<Variable>::regular(variable.clone()))
            }
            "variadic_parameter" => Ok(Parameter::<Variable>::variadic(None)),
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