use std::{path::PathBuf, sync::{Arc, Mutex}, vec};

use crate::{compiler::{diagnostics, strings}, driver, ir};

pub mod c;

pub mod doxygen;
pub mod cmark_comment;

pub mod actions;
pub mod arguments;

pub trait SymbolCommentIngestingFrontend {
    fn record_comment(
        &mut self, 
        text: &str, 
        prefix: &'static str, 
        location: &impl diagnostics::EditorLocation,
    ) {}

    fn enter_symbol_scope(&mut self,) {}
    fn leave_symbol_scope(&mut self) {}

    fn record_symbol(
        &mut self, 
        symbol: ir::Symbol,
        location: &impl diagnostics::EditorLocation,
    ) {}
}

pub trait MarkupIngestingFrontend {

}

#[derive(Debug, Clone)]
pub struct LanguageSettings {
    pub c: c::CLanguageSettings
}

#[derive(Debug, Clone)]
pub struct DialectConfig {
    pub doxygen: doxygen::DoxygenSettings
}

#[derive(Debug, thiserror::Error)]
pub enum LanguageError {
    #[error(transparent)]
    C(#[from] c::CLanguageError),

    #[error("The {0} programming language is not supported")]
    UnsupportedLanguage(ir::Language),
}

pub fn collect_language_symbols<D: diagnostics::DiagnosticReporter>(
    language: ir::Language,
    encoding: strings::Encoding,
    data: &[u8],
    config: &LanguageSettings,
    diags: &D
) -> Result<Vec<ir::Symbol>, LanguageError> {
    match language {
        ir::Language::C => {
            let mut symbols = vec![];
            c::gen_sema(encoding, data, &config.c, &mut symbols, diags)?;
            Ok(symbols)
        }
        _ => {
            Err(LanguageError::UnsupportedLanguage(language))
        }
    }
}