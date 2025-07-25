#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

use std::path::PathBuf;
use std::sync::{ Arc };
use crate::ir::rich_text::{ RichText };
use crate::frontend::c;

#[derive(Clone)]
pub enum Dialect {
    Doxygen,
    DocC,
    RustDoc,
    None
}

#[derive(Clone)]
pub enum Symbol {
    c(c::sema::Symbol),
}

#[derive(Clone)]
pub struct Role {
    pub id: [u8; 4],
    pub label: String
}

pub struct Property {
    pub id: String,
}

#[derive(Clone)]
pub struct Section {
    pub label: String,
    
    pub description: Option<RichText>,

    symbol_ixs: Vec<usize>
}

#[derive(Clone)]
pub struct Entry {
    role_ix: usize,

    pub title: String,

    pub abstract_: Option<RichText>,

    pub description: Option<RichText>,

    pub symbols: Vec<Symbol>, 

    pub sections: Vec<Section>,
}

pub struct EntryGraph {
    roles: Vec<Role>,
    entries: Vec<Entry>,
}

#[derive(Clone)]
pub enum EntryGraphError {

}

const DEFAULT_ENTRY_CAPACITY: usize = 1000;

impl EntryGraph {
    pub fn new() -> Self {
        Self {
            roles: vec![],
            entries: Vec::with_capacity(DEFAULT_ENTRY_CAPACITY),
        }
    }
    pub fn add(self: &mut Self, entry: Entry) -> Result<(), EntryGraphError>{
        self.entries.push(entry);
        Ok(())
    }
}