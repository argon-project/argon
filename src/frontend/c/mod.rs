use std::{convert::Infallible, str::Utf8Error, string::FromUtf16Error};

use crate::{compiler::{diagnostics, strings}, driver::Config};

pub mod ast;
pub mod sema_gen;
pub mod sema;
mod recorder;

#[derive(Debug, Clone)]
pub struct CLanguageSettings {
    pub sema_gen: sema_gen::Config,
}

#[derive(Debug, thiserror::Error)]
pub enum CLanguageError {
    #[error(transparent)]
    AST(#[from] ast::ASTError),

    #[error(transparent)]
    CSemaUTF8(#[from] sema_gen::SemaGenError<Utf8Error>),

     #[error(transparent)]
    CSemaUTF16(#[from] sema_gen::SemaGenError<FromUtf16Error>),
}

pub fn gen_sema<R: sema_gen::CLanguageRecorder<E>, E, D: diagnostics::DiagnosticReporter>(
    encoding: strings::Encoding,
    data: &[u8],
    config: &CLanguageSettings,
    witness: R,
    diags: &D
) -> Result<(), CLanguageError> {
    match encoding {
        strings::Encoding::UTF8 => {
        let ast = ast::gen_ast_utf8(data, None)?;
            sema_gen::ASTWalker::new(
                data, 
                &config.sema_gen, 
                witness, 
                diags
            ).gen_sema(ast.root_node())
            .map_err(|e| CLanguageError::CSemaUTF8(e))?;
        }
        strings::Encoding::UTF16(endianness) => {
            let (_, middle, _) = unsafe { data.align_to::<u16>() };
            let ast = ast::gen_ast_utf16(middle, None, endianness)?;
            sema_gen::ASTWalker::new(
                middle, 
                &config.sema_gen, 
                witness, 
                diags
            ).gen_sema(ast.root_node())
            .map_err(|e| CLanguageError::CSemaUTF16(e))?;
        }
    };
    Ok(())
}