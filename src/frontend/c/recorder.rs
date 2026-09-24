use std::{error::Error, vec};

use crate::{
    compiler::{
        diagnostics::{self, Location}, strings
    }, frontend::{
        self, SymbolCommentIngestingFrontend,
    }, ir,
};

use super::sema::{
    *
};

pub trait CLanguageRecorder<E> {
    fn record_include(&mut self, header: IncludedHeader) -> Result<bool, E> { Ok(false) }
    fn record_symbol(&mut self, symbol: Symbol, location: Location) -> Result<bool, E>;
    fn record_comment(&mut self, comment: &str, style: &'static str, location: Location) -> Result<bool, E> { Ok(false) }
    fn enter_scope(&mut self) {}
    fn leave_scope(&mut self) {}
    fn finish(&mut self) -> Result<(), E> { Ok(()) }
}

impl CLanguageRecorder<()> for Vec<ir::Symbol> {
    fn record_symbol(&mut self, symbol: Symbol, location: Location) -> Result<bool, ()> {
        self.push(ir::Symbol::c(symbol));
        Ok(true)
    }
}

impl CLanguageRecorder<()> for &mut Vec<ir::Symbol> {
    fn record_symbol(&mut self, symbol: Symbol, location: Location) -> Result<bool, ()> {
        self.push(ir::Symbol::c(symbol));
        Ok(true)
    }
}

impl CLanguageRecorder<()> for Vec<Symbol> {
    fn record_symbol(&mut self, symbol: Symbol, location: Location) -> Result<bool, ()> {
        self.push(symbol);
        Ok(true)
    }
}

impl CLanguageRecorder<()> for &mut Vec<Symbol> {
    fn record_symbol(&mut self, symbol: Symbol, location: Location) -> Result<bool, ()> {
        self.push(symbol);
        Ok(true)
    }
}

macro_rules! impl_for_symbol {
    ($( $variant:ident => $type:ident ),* $(,)?) => {
        $(
            impl CLanguageRecorder<()> for Vec<$type> {
                fn record_symbol(&mut self, symbol: Symbol, _location: Location) -> Result<bool, ()> {
                    let Symbol::$variant(item) = symbol else {
                        return Ok(false);
                    };
                    self.push(item);
                    Ok(true)
                }
            }

            impl CLanguageRecorder<()> for &mut Vec<$type> {
                fn record_symbol(&mut self, symbol: Symbol, _location: Location) -> Result<bool, ()> {
                    let Symbol::$variant(item) = symbol else {
                        return Ok(false);
                    };
                    self.push(item);
                    Ok(true)
                }
            }
        )*
    };
}

impl_for_symbol! {
    function => Function,
    variable => Variable,
    typeDefinition => TypeDefinition,
    enumeration => Enum,
    enumerationCase => EnumCase,
    macroDefintion => MacroDefinition,
}

pub struct CFrontend<F: SymbolCommentIngestingFrontend>(pub F);

impl<F: SymbolCommentIngestingFrontend> CLanguageRecorder<()> for CFrontend<F> {
    fn record_symbol(&mut self, symbol: Symbol, location: Location) -> Result<bool, ()> {
        self.0.record_symbol(ir::Symbol::c(symbol), &location);
        Ok(true)
    }

    fn record_comment(&mut self, comment: &str, style: &'static str, location: Location) -> Result<bool, ()> {
        self.0.record_comment(comment, style, &location);
        Ok(true)
    }

    fn enter_scope(&mut self) {
        self.0.enter_symbol_scope();
    }

    fn leave_scope(&mut self) {
        self.0.leave_symbol_scope();
    }

    fn record_include(&mut self, header: IncludedHeader) -> Result<bool, ()> {
        Ok(false)
    }
}
