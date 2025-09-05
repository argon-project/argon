use std::vec;

use crate::{
    compiler::{
        strings,
        diagnostics::Location
    },
    frontend::{
        self,
        DocumentationCommentRecorder,
    },
};

use super::sema::{
    IncludedHeader,
    Symbol,
};

pub trait Recorder<E> {
    fn record_include(&mut self, header: IncludedHeader) -> Result<bool, E>;
    fn record_symbol(&mut self, symbol: Symbol, location: Location) -> Result<bool, E>;
    fn record_comment(&mut self, comment: &str, style: &'static str, location: Location) -> Result<bool, E>;
    fn finish(&mut self) -> Result<(), E>;
}

impl Recorder<()> for Vec<Symbol> {
    fn record_include(self: &mut Self, _header: IncludedHeader) -> Result<bool, ()> {
        Ok(false)
    }

    fn record_symbol(self: &mut Self, symbol: Symbol, _location: Location) -> Result<bool, ()> {
        self.push(symbol);
        Ok(true)
    }

    fn record_comment(self: &mut Self, _comment: &str, _style: &'static str, _location: Location) -> Result<bool, ()> {
        Ok(false)
    }

    fn finish(self: &mut Self) -> Result<(), ()> {
        Ok(())
    }
}

impl Recorder<()> for Vec<IncludedHeader> {
    fn record_include(self: &mut Self, header: IncludedHeader) -> Result<bool, ()> {
        self.push(header);
        Ok(true)
    }

    fn record_symbol(self: &mut Self, _symbol: Symbol, _location: Location) -> Result<bool, ()> {
        Ok(false)
    }

    fn record_comment(self: &mut Self, _comment: &str, _style: &'static str, _location: Location) -> Result<bool, ()> {
        Ok(false)
    }

    fn finish(self: &mut Self) -> Result<(), ()> {
        Ok(())
    }
}

pub struct CRecorder<'a, R: DocumentationCommentRecorder> {
    next: &'a R,
    included_headers: Vec<IncludedHeader>
}

impl<'a, R: DocumentationCommentRecorder> CRecorder<'a, R> {
    pub fn new(next: &'a R) -> Self {
        Self {
            next,
            included_headers: vec![]
        }
    }
}


impl<'a, R: DocumentationCommentRecorder> Recorder<()> for CRecorder<'a, R> {
    fn record_symbol(&mut self, symbol: Symbol, location: Location) -> Result<bool, ()> {
        Ok(true)
    }

    fn record_include(&mut self, header: IncludedHeader) -> Result<bool, ()> {
        self.included_headers.push(header);
        Ok(true)
    }

    fn record_comment(&mut self, comment: &str, style: &'static str, location: Location) -> Result<bool, ()> {
        self.next.record_comment(comment, style);
        Ok(true)
    }

    fn finish(&mut self) -> Result<(), ()> {
        Ok(())
    }
}