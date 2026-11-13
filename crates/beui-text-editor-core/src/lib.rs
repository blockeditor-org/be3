mod changes;
mod core;
mod document;
mod highlighter;
pub mod html;
mod rich;
pub mod schema;

pub use be_lib_sequence::{Pos, SeqOp, Sequence, Splice};
pub use changes::{ChangeLog, TextChange};
pub use core::*;
pub use document::{
    Document, DocumentEdit, DocumentRead, DocumentView, TextBuffer, TextIndentation, TextLanguage,
    TextRun, anchor_in, anchor_index_in, changed, deleted_anchor_index_in, format_change,
    format_op, formats_in, place_in, remint, runs_in,
};
pub use highlighter::{
    Highlighter, Language, SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize,
    SyntaxHighlight,
};
pub use rich::{
    Block, Change, Expand, FormatOp, FormatRun, Formats, Mark, MarkId, MarkSet, Marks, Name,
    RichOp, Scope, Seed, VOID, Value,
};
pub use schema::{
    Break, Line, LineLook, ListMarker, Plain, Schema, Selected, TableLine, TableRow, Tool,
};

#[cfg(test)]
mod tests;
