use crate::compiler::diagnostics;

pub mod c {
    pub mod ast;
    pub mod sema_gen;
    pub mod sema;
    mod recorder;
}

pub mod doxygen;
pub mod cmark_comment;

pub mod actions;
pub mod arguments;

pub trait DocumentationCommentRecorder {
    fn record_comment(
        &mut self, 
        text: &str, 
        prefix: &'static str, 
        location: &impl diagnostics::EditorLocation,
        diags: &impl diagnostics::DiagnosticReporter
    );
}

pub trait DialectDriver {
    
}

pub trait LanguageDriver {

}