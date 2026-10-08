mod changes;
mod core;
mod document;
mod highlighter;

pub use changes::{ChangeLog, TextChange};
pub use core::*;
pub use document::{
    Document, DocumentEdit, DocumentRead, DocumentView, TextBuffer, TextIndentation, TextLanguage,
    anchor_in, anchor_index_in, changed, deleted_anchor_index_in,
};
pub use highlighter::{
    Highlighter, Language, MarkdownTable, MarkdownTableAlignment, MarkdownTableRow,
    SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize, SyntaxHighlight,
};
pub use sequence::{Pos, SeqOp, Sequence, Splice};

#[cfg(test)]
mod tests;
