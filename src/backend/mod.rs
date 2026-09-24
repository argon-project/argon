use serde::{
    Deserialize, Serialize
};
use strum_macros::Display;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Display, Deserialize, Serialize)]
pub enum Format {
    #[serde(rename = "html")]
    MostlyStaticHTML,

    #[serde(rename = "xml")]
    DoxyXML,

    #[serde(rename = "rtf")]
    RTF,

    #[serde(rename = "man")]
    ManPages,

    #[serde(rename = "latex")]
    LaTeX,

    #[serde(rename = "docbook")]
    DocBook,
}