pub mod backend;

pub mod frontend;

pub mod ir;

pub mod compiler {
    pub mod strings;
    pub mod diagnostics;

    pub type URL = url::Url;
}

pub mod driver;
