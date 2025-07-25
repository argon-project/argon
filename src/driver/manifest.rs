use serde::{
    Deserialize,
    Serialize
};

#[derive(Deserialize, Serialize, Debug)]
pub struct PreparsedManifest {
    pub version: semver::Version,
}

pub fn supported_manifest_versions() -> Vec<semver::Version> {
    vec![
        semver::Version::parse("1.0.0").unwrap()
    ]
}

pub mod v1 {
    use std::{
        collections::HashMap, path::PathBuf
    };
    use serde::{
        Deserialize,
        Serialize
    };
    use crate::{
        ir,
        backend
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

#[derive(Deserialize, Serialize, Debug)]
pub struct DocDoxygenDirective {
    pub name: String
}

#[derive(Deserialize, Serialize, Debug)]
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


#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "dialect")]
pub enum DocDialect {
    #[serde(rename = "doxygen")]
    Doxygen {
        #[serde(default)]
        #[serde(rename = "doxygenSettings")]
        settings: DocDoxygenSettings
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DocTarget {
    pub name: String,

    pub files: Vec<String>,

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

#[derive(Deserialize, Serialize, Debug)]
pub struct DocHTMLTheme {

}

impl Default for DocHTMLTheme {
    fn default() -> Self {
        Self {

        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
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


#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "format")]
pub enum DocProductFormat {
    #[serde(rename = "html")]
    MostlyStaticButPrettyHTML {
        #[serde(rename = "htmlSettings")]
        settings: DocHTMLSettings
    }
}

}