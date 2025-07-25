use pulldown_cmark::{
    Event
};
use crate::ir;

// In the IR, we don't need to know what the original 'base format' was.
// For instance, Doxygen, DocC, RustDoc, use Markdown as the base format.
// Here, we only care about how it is stored. If it is stored as
// pulldown-cmark events, then we only have to know how to handle those.
// As consequence, there may be multiple storage types for the same original
// format just due to different parser being used.
// If we're going to store something like Markdown as a raw text buffer in IR,
// then that would have its own storage type in RichTextStorage.
// The same logic also applies to the DocVariant. In IR, we don't care about details
// needed while parsing richt text containing documentation instructions.
// Now, we only care about what we expect when we handle what's in RichTextStorage.
// This might be necessary to handle behavior such as determining how links are to
// be resolved.

#[derive(Clone)]
enum RichTextStorage {
    CommonMarkEvents(Vec<Event<'static>>)
}

#[derive(Clone)]
pub struct RichText {
    dialect: ir::Dialect,

    storage: RichTextStorage,
}