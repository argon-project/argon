pub mod backend {

    use serde::{
        Deserialize, Serialize
    };
    use strum_macros::Display;

    #[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Display, Deserialize, Serialize)]
    pub enum Format {
        #[serde(rename = "html")]
        MostlyStaticHTML,
    }
}

pub mod frontend {
    pub mod c {
        pub mod ast;
        pub mod sema_gen;
        pub mod sema;
    }
    pub mod doxygen;
}

pub mod ir {
    pub(crate) mod rich_text;
    pub mod entry_graph;

    use serde::{
        Deserialize, Serialize
    };
    use strum_macros::Display;  

    #[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Display, Deserialize, Serialize)]
    pub enum Dialect {
        #[serde(rename = "doxygen")]
        Doxygen,

        #[serde(rename = "docc")]
        DocC,

        #[serde(rename = "rustdoc")]
        RustDoc,
    }

    #[derive(Clone, Copy, Hash, PartialEq, Eq, Deserialize, Serialize)]
    pub enum Language {
        #[serde(rename = "C", alias = "c")]
        C,
    }

    pub use entry_graph::EntryGraph;
    pub use entry_graph::Entry;
}

pub mod compiler {
    pub mod strings;
    pub mod diagnostics;
}

pub mod driver;