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
        collections::HashMap, path::PathBuf
    };
    use encoding::Encoding;
    use serde::{
        Deserialize,
        Serialize
    };
    use crate::{
        backend, compiler::{diagnostics::{self, DiagnosticReporter}, strings}, ir
    };

#[derive(Deserialize, Serialize, Debug)]
pub struct DocManifest {
    pub targets: Vec<DocTarget>,

    pub products: Vec<DocProduct>,
}

#[derive(Deserialize, Serialize, Debug)]
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

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "language")]
pub enum DocLanguage {
    #[serde(rename = "c")]
    C {
        #[serde(default)]
        #[serde(rename = "cSettings")]
        settings: DocCSettings
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DocDoxygenDirective {
    pub name: String
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
        #[serde(rename = "doxygenSettings")]
        settings: DocDoxygenSettings
    }
}

#[derive(Deserialize, Serialize, Debug)]
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

#[derive(Deserialize, Serialize, Debug)]
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

    #[error("Product '{0}' must specify targets because multiple targets exist")]
    MissingTargetsInProduct(String),

    #[error("Product '{0}' has no targets")]
    EmptyTargetList(String),

    #[error("Target '{0}' does not contain any files")]
    EmptyFileList(String),
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

impl DocManifest {
    pub fn proofread(&self, diags: &impl DiagnosticReporter) -> bool {
        let mut valid = false;
        for (ix, target) in self.targets.iter().enumerate() {
            valid &= target.proofread(ix, diags);
            if self.targets.len() > 1 {
                if target.name.is_none() {
                    valid = false;
                    diags.diagnose(
                        ManifestError::MissingTargetName(ix)
                    );
                }
            }
        }

        for (ix, product) in self.products.iter().enumerate() {
            valid &= product.proofread(ix, diags);
            if self.targets.len() > 1 {
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

        valid
    }
}

}