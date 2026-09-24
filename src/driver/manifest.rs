use serde::{
    Deserialize,
    Serialize,
    Deserializer,
    de
};
use std::fmt;
use lenient_semver;
use semver::Version;
use crate::{
    compiler::{
        diagnostics::{
            self,
            DiagnosticReporter
        }
    }
};

#[derive(Deserialize, Serialize, Debug)]
pub struct PreparsedManifest {
    #[serde(deserialize_with = "deserialize_lenient_version")]
    pub version: Version,
}

fn deserialize_lenient_version<'de, D: Deserializer<'de>>(
    deserializer: D
) -> Result<Version, D::Error> {
    struct VersionVisitor;

    impl<'de> de::Visitor<'de> for VersionVisitor {
        type Value = Version;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("semver version")
        }

        fn visit_str<E: de::Error>(self, string: &str) -> Result<Self::Value, E> {
            lenient_semver::parse(string).map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_str(VersionVisitor)
}

pub fn supported_manifest_versions() -> Vec<semver::Version> {
    vec![
        Version::parse("1.0.0").unwrap()
    ]
}

pub mod v1 {
    use std::{
        collections::{HashMap, HashSet}, fmt::Debug, path::PathBuf
    };
    use semver::Version;
    use serde::{
        Deserialize, Deserializer, Serialize, de::IntoDeserializer
    };
    use crate::{
        backend, 
        ir,
        frontend, 
        driver, 
        compiler::{
            URL, diagnostics::{
                self, 
                DiagnosticReporter
            }, strings
        }, 
    };

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocManifest {
    pub targets: Vec<DocTarget>,

    pub products: Vec<DocProduct>,

    #[serde(default)]
    pub model: DocModel,

    // #[serde(default)]
    // pub aspects: Vec<DocAspect>,

    #[serde(default)]
    pub scripts: Vec<DocScript>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DocModel {
    #[serde(default)]
    pub attributes: Vec<DocAttribute>,

    #[serde(default)]
    pub roles: Vec<DocRole>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocInliningBehavior {
    #[serde(default)]
    pub ordered: bool,
}

impl Into<ir::InliningBehavior> for DocInliningBehavior {
    fn into(self) -> ir::InliningBehavior {
        ir::InliningBehavior {
            ordered: self.ordered
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocRole {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub inlining_behavior: Option<DocInliningBehavior>,
    #[serde(rename = "associatedAttributes")]
    #[serde(default)]
    pub associated_attributes: Vec<String>
}

impl Into<ir::Role> for DocRole {
    fn into(self) -> ir::Role {
        ir::Role {
            label: self.label,
            inlining_behavior: self.inlining_behavior.map(|b| b.into()) 
        }
    }
}

impl DocManifest {
    pub fn roles(&self) -> ir::Roles {
        ir::Roles::from(
            self.model.roles
                .iter()
                .map(|a| 
                    (a.id.clone(), a.clone().into()))
        )
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum DocValueType {
    #[serde(rename = "markup")]
    RichText,

    #[serde(rename = "string")]
    String,
    
    #[serde(rename = "int")]
    Integer,
    
    #[serde(rename = "uint")]
    UnsignedInteger,

    /// Label + URI
    #[serde(rename = "link")]
    Link,

    /// Label + URI
    #[serde(rename = "uri")]
    URI,

    /// Attribute Present or not
    #[serde(rename = "boolean")]
    Boolean,

    /// Attribute Present or not
    #[serde(rename = "version")]
    Version,

    /// Attribute Present or not
    #[serde(rename = "reference")]
    Reference,
}

impl Default for DocValueType {
    fn default() -> Self {
        Self::String
    }
}

impl Into<ir::ValueType> for DocValueType {
    fn into(self) -> ir::ValueType {
        match self {
            Self::String => ir::ValueType::String,
            Self::Integer => ir::ValueType::SignedInteger,
            Self::UnsignedInteger => ir::ValueType::UnsignedInteger,
            Self::Link => ir::ValueType::Link,
            Self::Boolean => ir::ValueType::Boolean,
            Self::RichText => ir::ValueType::RichText,
            Self::URI => ir::ValueType::URI,
            Self::Version => ir::ValueType::Version,
            Self::Reference => ir::ValueType::Void,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum DocAttributeAppearance {
    #[serde(rename = "modifier")]
    Modifier,

    #[serde(rename = "annotation")]
    Annotation,

    #[serde(rename = "caption")]
    Caption,

    #[serde(rename = "admonition")]
    Admonition,

    #[serde(rename = "hidden")]
    Hidden,
}

impl Default for DocAttributeAppearance {
    fn default() -> Self {
        Self::Annotation
    }
}

impl Into<ir::AttributeAppearance> for DocAttributeAppearance {
    fn into(self) -> ir::AttributeAppearance {
        match self {
            Self::Modifier => ir::AttributeAppearance::Modifier,
            Self::Annotation => ir::AttributeAppearance::Annotation,
            Self::Caption => ir::AttributeAppearance::Caption,
            Self::Admonition => ir::AttributeAppearance::Admonition,
            Self::Hidden => ir::AttributeAppearance::Hidden
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocAttribute {
    pub id: String,
    pub label: String,

    #[serde(rename = "type", default)]
    pub value_type: DocValueType,
    
    #[serde(default)]
    pub repeatable: bool,

    #[serde(default)]
    pub appearance: DocAttributeAppearance,

    pub description: Option<String>,
}

impl Into<ir::Attribute> for DocAttribute {
    fn into(self) -> ir::Attribute {
        ir::Attribute {
            label: self.label.clone(),
            value_type: self.value_type.into(),
            repeatable: self.repeatable,
            appearance: self.appearance.into(),
            description: self.description.map(|d| ir::RichText::from_str(&d, ir::rich_text::RichTextRepresentation::CommonMark))
        }
    }
}

impl DocManifest {
    pub fn attributes(&self) -> ir::Attributes {
        ir::Attributes::from(
            self.model.attributes
                .iter()
                .map(|a| 
                    (a.id.clone(), a.clone().into()))
        )
    }
}

// #[derive(Deserialize, Serialize, Debug, Clone)]
// pub struct DocAspectItems {
//     #[serde(default)]
//     pub labelled: bool,

//     #[serde(default)]
//     pub referenced: bool,

//     #[serde(default)]
//     pub ordered: bool,
// }

// impl Default for DocAspectItems {
//     fn default() -> Self {
//         Self {
//             labelled: bool::default(),
//             referenced: bool::default(),
//             ordered: bool::default(),
//         }
//     }
// }

// #[derive(Deserialize, Serialize, Debug, Clone)]
// pub struct DocAspect {
//     pub id: String,
//     pub label: String,
    
//     pub items: DocAspectItems,

//     pub help: Option<String>,
// }

// impl Into<ir::Aspect> for DocAspect {
//     fn into(self) -> ir::Aspect {
//         ir::Aspect {
//             label: self.label.clone(),
//             items_labelled: self.items.labelled,
//             items_referenced: self.items.referenced,
//             items_ordered: self.items.ordered,
//             help: self.help.map(|d| RichText::from_str(&d, ir::rich_text::RichTextRepresentation::CommonMark))
//         }
//     }
// }

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocCSettings {
    #[serde(rename = "decayArrays", default)]
    pub decay_arrays: bool,

    #[serde(rename = "transformFunctionPointers", default)]
    pub transform_fptrs: bool,

    #[serde(rename = "ignoreHeaderDefines", default)]
    pub ignore_header_defines: bool,

    #[serde(rename = "defines", default)]
    pub defines: HashMap<String, isize>,
}

impl Default for DocCSettings {
    fn default() -> Self {
        Self {
            decay_arrays: false,
            transform_fptrs: true,
            ignore_header_defines: true,
            defines: HashMap::new()
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "language")]
pub enum DocLanguage {
    #[serde(rename = "c")]
    C {
        #[serde(default)]
        #[serde(rename = "cSettings")]
        settings: DocCSettings
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub enum DocDoxygenParameterLength {
    #[serde(rename = "word")]
    Word,

    #[serde(rename = "line")]
    Line,

    #[serde(rename = "paragraph")]
    Paragraph,
}

impl Default for DocDoxygenParameterLength {
    fn default() -> Self {
        Self::Word
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct DocDoxygenParameter {
    pub name: String,

    #[serde(default)]
    pub length: DocDoxygenParameterLength,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocDoxygenDirective {
    pub name: String,

    #[serde(default)]
    pub parameters: Vec<DocDoxygenParameter>,

    pub script: DocScriptSpecification,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocDoxygenSettings {
    pub directives: Vec<DocDoxygenDirective>
}
impl Default for DocDoxygenSettings {
    fn default() -> Self {
        Self {
            directives: vec![]
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, strum_macros::Display, strum_macros::AsRefStr)]
#[serde(tag = "dialect")]
pub enum DocDialect {
    #[serde(rename = "doxygen")]
    #[strum(to_string = "doxygen")]
    Doxygen {
        #[serde(default)]
        #[serde(rename = "doxygen")]
        settings: DocDoxygenSettings
    }
}

impl Into<ir::Dialect> for DocDialect {
    fn into(self) -> ir::Dialect {
        match self {
            Self::Doxygen { .. } => ir::Dialect::Doxygen
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocTarget {
    pub name: Option<String>,

    pub files: Vec<String>,

    #[serde(default)]
    pub encoding: strings::Encoding,

    #[serde(flatten)]
    pub language: Option<DocLanguage>,

    #[serde(flatten)]
    pub dialect: DocDialect,
}

impl Into<ir::Language> for DocLanguage {
    fn into(self) -> ir::Language {
        match self {
            Self::C { .. } => ir::Language::C,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocProduct {
    pub name: String,

    pub targets: Option<Vec<String>>,

    #[serde(flatten)]
    pub format: DocProductFormat
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocHTMLTheme {

}

impl Default for DocHTMLTheme {
    fn default() -> Self {
        Self {

        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocHTMLSettings {
    #[serde(rename = "idexHtmlForEveryEntry", default)]
    always_index_html: bool,

    #[serde(rename = "beautifyPaths", default)]
    beautify_paths: bool,

    #[serde(rename = "template", default)]
    template_path: Option<PathBuf>,

    #[serde(rename = "path", default = "DocHTMLSettings::default_output_path")]
    output_path: PathBuf,

    #[serde(rename = "optics", default)]
    theme: DocHTMLTheme
}

impl DocHTMLSettings {
    fn default_output_path() -> PathBuf {
        ".build/doc/html".into()
    }
}

impl Default for DocHTMLSettings {
    fn default() -> Self {
        Self {
            always_index_html: false,
            beautify_paths: true,
            template_path: None,
            output_path: DocHTMLSettings::default_output_path(),
            theme: DocHTMLTheme {  }
        }
    }
}


#[derive(Deserialize, Serialize, Debug, Clone, strum_macros::Display, strum_macros::AsRefStr)]
#[serde(tag = "format")]
pub enum DocProductFormat {
    #[serde(rename = "html")]
    #[strum(to_string = "html")]
    MostlyStaticButPrettyHTML {
        #[serde(rename = "htmlSettings")]
        settings: DocHTMLSettings
    }
}

const MANIFEST_CMARK_OPTIONS: pulldown_cmark::Options = pulldown_cmark::Options::empty()
    .union(pulldown_cmark::Options::ENABLE_DEFINITION_LIST)
    .union(pulldown_cmark::Options::ENABLE_GFM)
    .union(pulldown_cmark::Options::ENABLE_MATH)
    .union(pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION)
    .union(pulldown_cmark::Options::ENABLE_STRIKETHROUGH)
    .union(pulldown_cmark::Options::ENABLE_SUBSCRIPT)
    .union(pulldown_cmark::Options::ENABLE_SUPERSCRIPT)
    .union(pulldown_cmark::Options::ENABLE_TABLES);

#[derive(Clone, Debug)]
pub struct RichText {
    pub(crate) inner: ir::rich_text::CommonMark,
    pub(crate) source: String,
}

impl<'de> Deserialize<'de> for RichText {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let text = <&str>::deserialize(deserializer)?;
        let parser = pulldown_cmark::Parser::new_ext(text, MANIFEST_CMARK_OPTIONS);
        let events = parser.into_iter().map(|e| e.into_static()).collect();
        Ok(RichText { 
            inner: ir::rich_text::CommonMark::new(events),
            source: text.to_string()
        })
    }
}

impl Serialize for RichText {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        serializer.collect_str(self.source.as_str())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DocDoxygenBuiltinAction {
    #[serde(rename = "builtin.doxygen.fileinfo")]
    FileInfo,

    #[serde(rename = "builtin.doxygen.lineinfo")]
    LineInfo,

    #[serde(rename = "builtin.doxygen.file")]
    File,
}

const fn _true() -> bool { true }

#[derive(Clone, Debug)]
pub enum DocActionParameter<T> {
    Variable(String),
    Constant(T)
}

pub trait ExtractVar {
    fn extract(&self) -> Option<&str> { None }
}

impl<T> ExtractVar for DocActionParameter<T> {
    fn extract(&self) -> Option<&str> {
        self.as_variable()
    }
}

// Fallback for bool, String, i32, etc.
impl ExtractVar for bool {}
impl ExtractVar for backend::Format {}
impl ExtractVar for DocDoxygenBuiltinAction {}

impl<T: ExtractVar> ExtractVar for Option<T> {
    fn extract(&self) -> Option<&str> {
        if let Some(s) = self {
            s.extract()
        } else {
            None
        }
    }
}

impl<T> DocActionParameter<T> {
    pub fn as_variable(&self) -> Option<&str> {
        match self {
            DocActionParameter::Variable(name) => Some(name),
            _ => None,
        }
    }
}

impl<'de, T> Deserialize<'de> for DocActionParameter<T> where T: Deserialize<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        // Helper enum to distinguish between raw strings (variables) and type T
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper<T> {
            Str(String),
            Typed(T),
        }

        match Helper::<T>::deserialize(deserializer)? {
            Helper::Str(s) => {
                if let Some(var_name) = s.strip_prefix('$') {
                    Ok(Self::Variable(var_name.to_string()))
                } else {
                    Ok(Self::Constant(
                        T::deserialize(s.into_deserializer())?)
                    )
                }
            }
            Helper::Typed(t) => Ok(Self::Constant(t)),
        }
    }
}

impl<T> Serialize for DocActionParameter<T> where T: Serialize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        match self {
            Self::Variable(variable) => serializer.serialize_str(("$".to_string() + variable).as_str()),
            Self::Constant(t) => t.serialize(serializer)
        }
    }
}

macro_rules! define_doc_actions {
    (
        $(#[$enum_meta:meta])*
        pub enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident { 
                    $(
                        $(#[$field_meta:meta])* $field:ident : $ty:ty
                    ),* $(,)?
                }
            ),* $(,)?
        }
    ) => {
        // 1. Generate the Enum Definition
        $(#[$enum_meta])*
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant {
                    $(
                        $(#[$field_meta])* $field : $ty
                    ),*
                }
            ),*
        }

        // 2. Generate the Implementation
        impl $name {
            pub fn variables(&self) -> Vec<&str> {
                match self {
                    $(
                        Self::$variant { $($field),* } => {
                            // Using the ExtractVar trait here allows us 
                            // to call .extract() on bools and Parameters alike.
                            vec![ $($field.extract()),* ]
                                .into_iter()
                                .flatten()
                                .collect()
                        }
                    ),*
                }
            }
        }
    };
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DocValue {
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

    Entry(String)
}

define_doc_actions! {
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub enum DocAction {

        #[serde(rename = "entry")]
        CreateEntry {
            id: DocActionParameter<String>,
            title: DocActionParameter<String>,
            content: DocActionParameter<Option<RichText>>,
            #[serde(default = "_true")]
            continue_content: bool 
        },



        /// Creates a member section with members of the scoped opened next
        #[serde(rename = "section")]
        CreateMemberSection {
            title: DocActionParameter<String>,

            content: DocActionParameter<Option<RichText>>,

            continue_content: bool,
        },

        /// Moves content of the scope opened next to entry with specified ID
        #[serde(rename = "scope.teleport")]
        Teleport {
            entry_id: DocActionParameter<String>,

            content: DocActionParameter<Option<RichText>>,

            continue_content: bool,
        },

        /// Indicates membership of current entry
        /// 
        /// If membership is added multiple times, the first membership constitues ownership.
        #[serde(rename = "entry.memberships.add")]
        AddMembership {
            entry_id: DocActionParameter<String>,
        },

        /// Sets abstract text of current entry
        #[serde(rename = "entry.abstract.set")]
        SetAbstract {
            content: DocActionParameter<RichText>
        },

        /// Sets attribute on current entry
        #[serde(rename = "entry.attributes.add")]
        AddAttribute {
            key: DocActionParameter<String>,
            value: DocActionParameter<DocValue>,
        },

        /// Sets role of current entry
        #[serde(rename = "entry.role.set")]
        SetRole {
            role: DocActionParameter<String>
        },

        /// Sets this entry as the root entry
        #[serde(rename = "graph.root.set")]
        SetRootEntry { a: bool },
        
        /// Parses, generates sema, and attaches symbol to current entry.
        /// 
        /// To create a new entry with this symbol, invoke [``Action::CreateEntry``]
        /// before. And set its role using [``Action::SetRole``] to [``ir::Role``]
        #[serde(rename = "symbol")]
        AddSymbol {
            raw: DocActionParameter<String>,
            language: DocActionParameter<ir::Language>
        },
        
        /// Sets symbol heading of current symbol
        #[serde(rename = "symbol.heading.set")]
        SetSymbolHeading {
            content: DocActionParameter<String>
        },
        
        /// Sets symbol help text of current symbol
        #[serde(rename = "symbol.help.set")]
        SetSymbolHelp {
            content: DocActionParameter<RichText>
        },
        
        /// Assigns current symbol to other entry // TODO: sounds like a builtin
        #[serde(rename = "symbol.attach")]
        AssignSymbol {
            entry_id: DocActionParameter<String>,
        },

        /// Inserts rich text
        #[serde(rename = "markup.admonition")]
        InsertAdmonition {
            contents: DocActionParameter<RichText>,
            kind: DocActionParameter<ir::AdmonitionKind>
        },

        #[serde(rename = "markup.text")]
        InsertText {
            text: DocActionParameter<String>,
        },

        #[serde(rename = "markup.block")]
        Insert {
            format: Option<backend::Format>,
            content: DocActionParameter<Option<RichText>>,
            continue_content: bool,
        },

        /// Insert into specified output from file
        #[serde(rename = "markup.include")]
        InsertFile {
            format: Option<backend::Format>,
            file: DocActionParameter<PathBuf>
        },

        #[serde(rename = "markup.link")]
        /// Inserts reference to another entry with specified id
        InsertReference {
            entry_id: DocActionParameter<String>
        },

        #[serde(untagged)]
        DoxygenBuiltin {
            #[serde(flatten)]
            builtin: DocDoxygenBuiltinAction 
        },
    }
}

use serde_variant::to_variant_name;

impl DocAction {
    pub fn id(&self) -> &'static str {
        match self {
            Self::DoxygenBuiltin { builtin } => to_variant_name(builtin).unwrap(),
            _ => to_variant_name(self).unwrap()
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum DocScriptSpecification {
    Named(String),
    Anonymous(Vec<DocAction>)
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocScript {
    pub name: String,
    pub actions: Vec<DocAction>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("The {0} dialect specified in target {1} is not supported")]
    UnsupportedDialect(DocDialect, String),

     #[error("The {0} output format specified in product {1} is not supported")]
    UnsupportedFormat(DocProductFormat, String),

    #[error("The target '{0}' reference in product '{1}' does not exist")]
    DanglingTargetReference(String, String),

    #[error("Target at index {0} must have a name because multiple target exists")]
    MissingTargetName(usize),

    #[error("Duplicate targets with name '{0}'")]
    DuplicateTargetName(String),

    #[error("Duplicate products with name '{0}'")]
    DuplicateProductName(String),

    #[error("Duplicate script with name '{0}'")]
    DuplicateScriptName(String),

    #[error("Duplicate attribute with identifier '{0}'")]
    DuplicateAtributeID(String),

    #[error("Duplicate role with identifier '{0}'")]
    DuplicateRoleID(String),

    #[error("Undefined attribute '{0}' associated with role '{1}'")]
    UnknownAssociatedRoleAttribute(String, String),

    #[error("Product '{0}' must specify targets because multiple targets exist")]
    MissingTargetsInProduct(String),

    #[error("Product '{0}' has no targets")]
    EmptyTargetList(String),

    #[error("Target '{0}' does not contain any files")]
    EmptyFileList(String),

    #[error("Doxygen directive name '{0}' is reserved by built-in directive")]
    ReservedDoxygenDirectiveName(String),

    #[error("Doxygen directive name '{0}' is invalid, only A-Z, a-z, and 0-9 allowed")]
    InvalidDoxygenDirectiveName(String),

    #[error("Doxygen directive parameter '{0}' defined multiple times")]
    DuplicateDoxygenDirectiveParameter(String),

    #[error("Doxygen directive word-long parameter '{0}' must not occur after line-long or paragraph-long parameter")]
    DoxygenWordParameterAfterLineOrParagraph(String),

    #[error("Doxygen directive parameter '{0}' must be trailing and there must not be more than one line-long or paragraph-long parameter")]
    NonUniqueTrailingLineOrParagraphDoxygenDirectiveParameter(String),

    #[error("Script '{0}' definition is missing")]
    MissingScript(String),

    #[error("Parameter '{0}' referenced in '{1}' action in script for '{2}' directive is not a defined parameter in '{2}'")]
    UndefinedArgumentReferenceInScript(String, String, String),
}

impl diagnostics::Diagnostic for ManifestError {
    fn severity(&self) -> diagnostics::Severity {
        match self {
            Self::EmptyTargetList(_) |
            Self::EmptyFileList(_)
             => diagnostics::Severity::Warning,
            _ => diagnostics::Severity::Error
        }
    }
}

impl DocTarget {
    pub fn proofread(&self, ix: usize, diags: &impl DiagnosticReporter) -> bool {
        let mut valid = true;
        match &self.dialect {
            DocDialect::Doxygen {
                ..
            } => {},
            dialect => {
                valid = false;
                diags.diagnose(
                    ManifestError::UnsupportedDialect(
                        dialect.clone(), 
                        self.name.clone().unwrap_or(ix.to_string())
                    )
                );
            }
        };

        if self.files.is_empty() {
            // This is not a reason to fail
            diags.diagnose(
                ManifestError::EmptyFileList(self.name.clone().unwrap_or(ix.to_string()))
            );
        }

        valid
    }
}

impl DocProduct {
    pub fn proofread(&self, ix: usize, diags: &impl DiagnosticReporter) -> bool {
        let mut valid = true;
        match &self.format {
            DocProductFormat::MostlyStaticButPrettyHTML {
                ..
            } => {},
            dialect => {
                valid = false;
                diags.diagnose(
                    ManifestError::UnsupportedFormat(
                        self.format.clone(), 
                        self.name.clone()
                    )
                );
            }
        };
        valid
    }
}

impl DocDoxygenDirective {
    pub fn proofread(&self, ix: usize, scripts: &[DocScript], diags: &impl DiagnosticReporter) -> bool {
        // Name...
        let mut valid = true;
        if frontend::doxygen::predefined::builtins().contains_key(&self.name) {
            valid = false;
            diags.diagnose(
                ManifestError::ReservedDoxygenDirectiveName(self.name.clone())
            );
        }
        else if self.name.chars().all(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9')) {
            valid = false;
            diags.diagnose(
                ManifestError::InvalidDoxygenDirectiveName(self.name.clone())
            );
        }

        // Parameters...
        let mut param_names: HashSet<&str> = HashSet::new();
        let last_param_length = DocDoxygenParameterLength::Word;
        for param in &self.parameters {
            if param_names.contains(param.name.as_str()) {
                valid = false;
                diags.diagnose(
                    ManifestError::DuplicateDoxygenDirectiveParameter(param.name.clone())
                );
            }
            param_names.insert(param.name.as_str());

            match param.length {
                DocDoxygenParameterLength::Word => {
                    if last_param_length != param.length {
                        valid = false;
                        diags.diagnose(
                            ManifestError::DoxygenWordParameterAfterLineOrParagraph(param.name.clone())
                        );
                    }
                }
                DocDoxygenParameterLength::Line |
                DocDoxygenParameterLength::Paragraph => {
                    if last_param_length != DocDoxygenParameterLength::Word {
                        valid = false;
                        diags.diagnose(
                            ManifestError::NonUniqueTrailingLineOrParagraphDoxygenDirectiveParameter(param.name.clone())
                        );
                    }
                }
            }
        }

        let actions = match &self.script {
            DocScriptSpecification::Anonymous(actions) => actions,
            DocScriptSpecification::Named(name) => {
                let script = scripts.iter().find(|s| s.name.as_str() == name.as_str());

                if let Some(script) = script {
                    &script.actions
                } else {
                    valid = false;
                    diags.diagnose(
                        ManifestError::MissingScript(name.clone())
                    );
                    &vec![]
                }
            }
        };

        for action in actions {
            for param_name in action.variables() {
                if !param_names.contains(param_name) {
                    valid = false;
                    diags.diagnose(
                        ManifestError::UndefinedArgumentReferenceInScript(
                            param_name.to_string(), 
                            action.id().to_string(), 
                            self.name.clone())
                    );
                }
            }
        }

        valid
    }
}

impl DocScript {
    pub fn proofread(&self, ix: usize, diags: &impl DiagnosticReporter) -> bool {
        let mut valid = true;
        
        valid
    }
}

impl DocManifest {
    pub fn proofread(&self, diags: &impl DiagnosticReporter) -> bool {
        let mut valid = true;
        let mut target_names = HashSet::new();
        for (ix, target) in self.targets.iter().enumerate() {
            valid &= target.proofread(ix, diags);
            if self.targets.len() > 1 {
                if let Some(name) = &target.name {
                    if target_names.contains(name.as_str()) {
                        valid = false;
                        diags.diagnose(
                            ManifestError::DuplicateTargetName(name.clone())
                        );
                    }
                    target_names.insert(name.as_str());
                } else {
                    valid = false;
                    diags.diagnose(
                        ManifestError::MissingTargetName(ix)
                    );
                }
            }
        }

        let mut product_names = HashSet::new();
        for (ix, product) in self.products.iter().enumerate() {
            valid &= product.proofread(ix, diags);
            if self.targets.len() > 1 {
                if product_names.contains(product.name.as_str()) {
                    valid = false;
                    diags.diagnose(
                        ManifestError::DuplicateProductName(product.name.clone())
                    );
                }
                product_names.insert(product.name.as_str());

                match &product.targets {
                    None => {
                        valid = false;
                        diags.diagnose(
                            ManifestError::MissingTargetsInProduct(product.name.clone())
                        );
                    }
                    Some(targets) => {
                        if targets.is_empty() {
                            // This is not a reason to fail
                            diags.diagnose(
                                ManifestError::EmptyTargetList(product.name.clone())
                            );
                        }
                    }
                }

            }
        }

        let mut script_names = HashSet::new();
        for (ix, script) in self.scripts.iter().enumerate() {
            valid &= script.proofread(ix, diags);
            if script_names.contains(script.name.as_str()) {
                valid = false;
                diags.diagnose(
                    ManifestError::DuplicateScriptName(script.name.clone())
                );
            }
            script_names.insert(script.name.as_str()); 
        }

        let mut attribute_names = HashSet::new();
        for (ix, attribute) in self.model.attributes.iter().enumerate() {
            if attribute_names.contains(attribute.id.as_str()) {
                valid = false;
                diags.diagnose(
                    ManifestError::DuplicateAtributeID(attribute.id.clone())
                );
            }
            attribute_names.insert(attribute.id.as_str()); 
        }

        let mut role_names = HashSet::new();
        for (ix, role) in self.model.roles.iter().enumerate() {
            if role_names.contains(role.id.as_str()) {
                valid = false;
                diags.diagnose(
                    ManifestError::DuplicateRoleID(role.id.clone())
                );
            }
            role_names.insert(role.id.as_str()); 
            for assoc_attr in role.associated_attributes.iter() {
                if !attribute_names.contains(assoc_attr.as_str()) {
                    valid = false;
                    diags.diagnose(
                        ManifestError::UnknownAssociatedRoleAttribute(
                            assoc_attr.clone(), 
                            role.id.clone())
                    );
                }
            }
        }

        valid
    }
}

impl DocManifest {
    pub fn targets(&self) -> impl Iterator<Item = driver::Target> {
        self.targets
            .iter()
            .enumerate()
            .map(|(ix, t)| driver::Target {
                name: t.name.clone().unwrap_or_else(|| ix.to_string()),
                files: t.files.clone(),
                dialect: t.dialect.clone().into(),
                language: t.language.clone().map(|l| l.into()),
                encoding: t.encoding,
                dialect_config: frontend::DialectConfig {
                    doxygen: frontend::doxygen::DoxygenSettings {
                        directives: frontend::doxygen::predefined::builtins()
                    }
                },
                language_config: frontend::LanguageSettings { 
                    c: frontend::c::CLanguageSettings {
                        sema_gen: frontend::c::sema_gen::Config::default()
                    }
                },
            })
    }
}

}