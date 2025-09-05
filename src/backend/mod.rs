use serde::{
    Deserialize, Serialize
};
use strum_macros::Display;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Display, Deserialize, Serialize)]
pub enum Format {
    #[serde(rename = "html")]
    MostlyStaticHTML,
}