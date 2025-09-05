use tree_sitter::{
    LanguageError, ParseOptions, Parser, Tree,
};
use tree_sitter_c;
use thiserror;
use crate::compiler::{
    diagnostics,
    strings
};

#[derive(Debug, thiserror::Error)]
pub enum ASTError {
    #[error("Error loading language grammar: {0}")]
    LanguageFailure(#[from] #[source] LanguageError),

    #[error("C source code is not parsable with built-in grammar")]
    UnparsableSource, 
}

impl diagnostics::Diagnostic for ASTError {}

fn _parser() -> Result<Parser, LanguageError> {
    let mut parser = Parser::new();

    parser
        .set_language(&tree_sitter_c::LANGUAGE.into())?;

    Ok(parser)
}


pub fn gen_ast_utf8<'a, S: strings::Source<CodeUnit = u8>>(
    source: S,
    options: Option<ParseOptions>
) -> Result<Tree, ASTError> {
    let mut parser: Parser = _parser()?;

    parser
        .parse_with_options(
            &mut |i, p| source.encoded_slice(i, p), 
            None, 
            options
        )
        .ok_or(ASTError::UnparsableSource)
}

pub fn gen_ast_utf16<'a, S: strings::Source<CodeUnit = u16>>(
    source: S,
    options: Option<ParseOptions>,
    endianness: strings::Endianness
) -> Result<Tree, ASTError> {
    let mut parser: Parser = _parser()?;

    let tree =
    match endianness {
        strings::Endianness::BigEndian => {
            parser.parse_utf16_be_with_options(
                &mut |i, p| source.encoded_slice(i, p), 
                None, 
                options
            )
        }
        strings::Endianness::LittleEndian => {
            parser.parse_utf16_le_with_options(
                &mut |i, p| source.encoded_slice(i, p), 
                None, 
                options
            )
        }
    };

    tree.ok_or(ASTError::UnparsableSource)
}