use std::ops::Range;
use std::sync::{Arc, Mutex};

use tree_sitter::{InputEdit, Parser, Point, Tree};

use crate::TextChange;
use crate::document::{Document, DocumentView, TextLanguage};

mod markdown;
mod rust;
mod zig;

pub use markdown::{MarkdownTable, MarkdownTableAlignment, MarkdownTableRow};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Markdown,
    Rust,
    Zig,
}

impl Language {
    pub const fn for_document(language: TextLanguage) -> Option<Self> {
        match language {
            TextLanguage::PlainText => None,
            TextLanguage::Markdown => Some(Self::Markdown),
            TextLanguage::Rust => Some(Self::Rust),
            TextLanguage::Zig => Some(Self::Zig),
        }
    }

    fn chain_start_offset(self, kind: &str, source: &[u8]) -> usize {
        match self {
            Self::Markdown => 0,
            Self::Rust => rust::chain_start_offset(kind, source),
            Self::Zig => zig::chain_start_offset(kind, source),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynHlColorScope {
    Invalid,
    PunctuationImportant,
    Punctuation,
    VariableFunction,
    VariableParameter,
    VariableConstant,
    VariableMutable,
    Variable,
    LiteralString,
    Literal,
    KeywordStorage,
    KeywordPrimitiveType,
    Keyword,
    Comment,
    MarkdownPlainText,
    MarkdownSymbol,
    MarkdownLink,
    MarkdownCode,
    Unstyled,
    Invisible,
}

impl SynHlColorScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::PunctuationImportant => "punctuation_important",
            Self::Punctuation => "punctuation",
            Self::VariableFunction => "variable_function",
            Self::VariableParameter => "variable_parameter",
            Self::VariableConstant => "variable_constant",
            Self::VariableMutable => "variable_mutable",
            Self::Variable => "variable",
            Self::LiteralString => "literal_string",
            Self::Literal => "literal",
            Self::KeywordStorage => "keyword_storage",
            Self::KeywordPrimitiveType => "keyword_primitive_type",
            Self::Keyword => "keyword",
            Self::Comment => "comment",
            Self::MarkdownPlainText => "markdown_plain_text",
            Self::MarkdownSymbol => "markdown_symbol",
            Self::MarkdownLink => "markdown_link",
            Self::MarkdownCode => "markdown_code",
            Self::Unstyled => "unstyled",
            Self::Invisible => "invisible",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SynHlFontFamily {
    #[default]
    Proportional,
    Monospace,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SynHlTextSize {
    #[default]
    Body,
    Heading(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SynHlStyle {
    pub color: SynHlColorScope,
    pub family: SynHlFontFamily,
    pub size: SynHlTextSize,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl SynHlStyle {
    const fn plain(color: SynHlColorScope) -> Self {
        Self {
            color,
            family: SynHlFontFamily::Proportional,
            size: SynHlTextSize::Body,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
        }
    }
}

pub struct SyntaxHighlight {
    styles: Styles,
}

enum Styles {
    Uniform { style: SynHlStyle, len: usize },
    Each(Vec<SynHlStyle>),
    Markdown(Arc<MarkdownSource>),
}

struct MarkdownSource {
    bytes: Arc<[u8]>,
    fences: Vec<Range<usize>>,
    table_starts: Vec<usize>,
    windows: Mutex<Vec<(usize, Arc<markdown::MarkdownWindow>)>>,
}

const MARKDOWN_WINDOWS_KEPT: usize = 16;
const MARKDOWN_WINDOW_LIMIT: usize = 4 * markdown::WINDOW_TARGET;

impl MarkdownSource {
    fn new(bytes: Arc<[u8]>) -> Self {
        Self {
            fences: markdown::fences(&bytes),
            table_starts: markdown::table_starts(&bytes),
            bytes,
            windows: Mutex::default(),
        }
    }

    fn window(&self, index: usize) -> (usize, Arc<markdown::MarkdownWindow>) {
        let mut windows = self
            .windows
            .lock()
            .expect("the markdown windows were poisoned");
        if let Some((start, window)) = windows
            .iter()
            .find(|(start, window)| (*start..*start + window.len).contains(&index))
        {
            return (*start, Arc::clone(window));
        }
        let range = markdown::window_range(&self.bytes, &self.fences, index..index + 1);
        let window = Arc::new(markdown::parse_window(&self.bytes[range.clone()]));
        if windows.len() >= MARKDOWN_WINDOWS_KEPT {
            windows.remove(0);
        }
        windows.push((range.start, Arc::clone(&window)));
        (range.start, window)
    }

    fn successor(&self, bytes: Arc<[u8]>, change: TextChange) -> Self {
        let fences = markdown::shifted_fences(&self.bytes, &self.fences, &bytes, change);
        let table_starts =
            markdown::shifted_table_starts(&self.bytes, &self.table_starts, &bytes, change);
        let fences_kept = fences.len() == self.fences.len()
            && self.fences.iter().zip(&fences).all(|(old, new)| {
                change.moved(old.start) == Some(new.start) && change.moved(old.end) == Some(new.end)
            });
        let touched = markdown::touched_lines(&self.bytes, change.start, change.old_end);
        let windows = match fences_kept {
            false => Vec::new(),
            true => self
                .windows
                .lock()
                .expect("the markdown windows were poisoned")
                .iter()
                .filter_map(|(start, window)| {
                    let end = start + window.len;
                    if end < touched.start {
                        return Some((*start, Arc::clone(window)));
                    }
                    if *start >= touched.end {
                        return Some((change.moved(*start)?, Arc::clone(window)));
                    }
                    if !markdown::holds_change(&self.bytes, *start..end, change) {
                        return None;
                    }
                    let new_end = end - change.old_end + change.new_end;
                    if new_end - start > MARKDOWN_WINDOW_LIMIT {
                        return None;
                    }
                    let inner = TextChange {
                        start: change.start - start,
                        old_end: change.old_end - start,
                        new_end: change.new_end - start,
                    };
                    let reparsed = markdown::reparse_window(
                        window,
                        &self.bytes[*start..end],
                        &bytes[*start..new_end],
                        inner,
                    );
                    Some((*start, Arc::new(reparsed)))
                })
                .collect(),
        };
        Self {
            bytes,
            fences,
            table_starts,
            windows: Mutex::new(windows),
        }
    }
}

impl SyntaxHighlight {
    pub(crate) fn plaintext(len: usize) -> Self {
        Self {
            styles: Styles::Uniform {
                style: SynHlStyle::plain(SynHlColorScope::Unstyled),
                len,
            },
        }
    }

    fn from_scopes(mut scopes: Vec<SynHlColorScope>, bytes: &[u8]) -> Self {
        for index in 1..scopes.len() {
            if bytes[index].is_ascii_whitespace() {
                scopes[index] = scopes[index - 1];
            }
        }
        Self {
            styles: Styles::Each(scopes.into_iter().map(SynHlStyle::plain).collect()),
        }
    }

    fn markdown(source: Arc<MarkdownSource>) -> Self {
        Self {
            styles: Styles::Markdown(source),
        }
    }

    pub fn advance_and_read(&self, byte_index: usize) -> SynHlColorScope {
        self.style_at(byte_index).color
    }

    pub fn style_at(&self, byte_index: usize) -> SynHlStyle {
        let invalid = SynHlStyle::plain(SynHlColorScope::Invalid);
        match &self.styles {
            Styles::Uniform { style, len } => match byte_index < *len {
                true => *style,
                false => invalid,
            },
            Styles::Each(styles) => styles.get(byte_index).copied().unwrap_or(invalid),
            Styles::Markdown(source) => {
                if byte_index >= source.bytes.len() {
                    return invalid;
                }
                let (start, window) = source.window(byte_index);
                window
                    .styles
                    .get(byte_index - start)
                    .copied()
                    .unwrap_or(invalid)
            }
        }
    }

    pub fn styles_in(&self, range: Range<usize>) -> Vec<SynHlStyle> {
        let Styles::Markdown(source) = &self.styles else {
            return range.map(|index| self.style_at(index)).collect();
        };
        let end = range.end.min(source.bytes.len());
        let mut styles = Vec::with_capacity(end.saturating_sub(range.start));
        let mut at = range.start;
        while at < end {
            let (start, window) = source.window(at);
            let until = end.min(start + window.len);
            styles.extend_from_slice(&window.styles[at - start..until - start]);
            at = until;
        }
        styles
    }

    pub fn markdown_tables(&self) -> Vec<MarkdownTable> {
        let Styles::Markdown(source) = &self.styles else {
            return Vec::new();
        };
        let mut tables: Vec<MarkdownTable> = Vec::new();
        for &start in &source.table_starts {
            let (window_start, window) = source.window(start);
            for table in &window.tables {
                let first = table.rows.first().map(|row| row.range.start + window_start);
                if first == Some(start)
                    && !tables
                        .iter()
                        .any(|held| held.rows.first().map(|row| row.range.start) == first)
                {
                    tables.push(markdown::shift_table(table, window_start));
                }
            }
        }
        tables
    }

    pub fn in_code_block(&self, range: Range<usize>) -> bool {
        let Styles::Markdown(source) = &self.styles else {
            return false;
        };
        if range.start >= source.bytes.len() {
            return false;
        }
        let (start, window) = source.window(range.start);
        window
            .code_blocks
            .iter()
            .any(|block| block.start + start <= range.end && block.end + start > range.start)
    }
}

enum ParserBackend {
    TreeSitter { parser: Parser, tree: Option<Tree> },
    Markdown,
}

impl ParserBackend {
    fn tree_sitter(language: &tree_sitter::Language) -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(language)
            .expect("tree-sitter language is incompatible");
        Self::TreeSitter { parser, tree: None }
    }
}

pub struct Highlighter {
    markdown: Option<(u64, Arc<MarkdownSource>)>,
    document: Arc<dyn Document>,
    backend: ParserBackend,
    parsed_revision: Option<u64>,
    parsed_bytes: Vec<u8>,
    language: Language,
}

impl Highlighter {
    pub fn new(document: Arc<dyn Document>, language: Language) -> Self {
        let backend = match language {
            Language::Markdown => ParserBackend::Markdown,
            Language::Rust => ParserBackend::tree_sitter(&tree_sitter_rust::LANGUAGE.into()),
            Language::Zig => ParserBackend::tree_sitter(&tree_sitter_zig::LANGUAGE.into()),
        };
        Self {
            markdown: None,
            document,
            backend,
            parsed_revision: None,
            parsed_bytes: Vec::new(),
            language,
        }
    }

    fn ensure_parsed(&mut self) -> Option<()> {
        let revision = self.document.revision();
        let has_tree = match &self.backend {
            ParserBackend::TreeSitter { tree, .. } => tree.is_some(),
            ParserBackend::Markdown => return Some(()),
        };
        if self.parsed_revision == Some(revision) && has_tree {
            return Some(());
        }
        let read = self.document.read()?;
        let document = DocumentView::new(&*read);
        let bytes = document.bytes();
        let logged = self
            .parsed_revision
            .and_then(|parsed| self.document.changes_since(parsed));
        let edit = if let Some(change) = logged {
            Some(InputEdit {
                start_byte: change.start,
                old_end_byte: change.old_end,
                new_end_byte: change.new_end,
                start_position: byte_point(&self.parsed_bytes, change.start),
                old_end_position: byte_point(&self.parsed_bytes, change.old_end),
                new_end_position: byte_point(bytes, change.new_end),
            })
        } else if self.parsed_revision.is_some() {
            let prefix = self
                .parsed_bytes
                .iter()
                .zip(bytes)
                .take_while(|(left, right)| left == right)
                .count();
            let maximum_suffix = self.parsed_bytes.len().min(bytes.len()) - prefix;
            let suffix = self
                .parsed_bytes
                .iter()
                .rev()
                .zip(bytes.iter().rev())
                .take(maximum_suffix)
                .take_while(|(left, right)| left == right)
                .count();
            let old_end = self.parsed_bytes.len() - suffix;
            let new_end = bytes.len() - suffix;
            Some(InputEdit {
                start_byte: prefix,
                old_end_byte: old_end,
                new_end_byte: new_end,
                start_position: byte_point(&self.parsed_bytes, prefix),
                old_end_position: byte_point(&self.parsed_bytes, old_end),
                new_end_position: byte_point(bytes, new_end),
            })
        } else {
            None
        };
        match &mut self.backend {
            ParserBackend::TreeSitter { parser, tree } => {
                if let (Some(tree), Some(edit)) = (tree.as_mut(), edit.as_ref()) {
                    tree.edit(edit);
                }
                *tree = parser.parse(bytes, tree.as_ref());
            }
            ParserBackend::Markdown => {}
        }
        self.parsed_bytes.clear();
        self.parsed_bytes.extend_from_slice(bytes);
        self.parsed_revision = Some(revision);
        Some(())
    }

    pub fn highlight(&mut self) -> SyntaxHighlight {
        if self.language == Language::Markdown {
            let revision = self.document.revision();
            if let Some((held, source)) = &self.markdown
                && *held == revision
            {
                return SyntaxHighlight::markdown(Arc::clone(source));
            }
            let Some(read) = self.document.read() else {
                return SyntaxHighlight::plaintext(0);
            };
            let bytes: Arc<[u8]> = read.slice(0..read.len()).into();
            drop(read);
            let change = self
                .markdown
                .as_ref()
                .and_then(|(held, _)| self.document.changes_since(*held));
            let source = Arc::new(match (self.markdown.take(), change) {
                (Some((_, previous)), Some(change)) => previous.successor(bytes, change),
                _ => MarkdownSource::new(bytes),
            });
            self.markdown = Some((revision, Arc::clone(&source)));
            return SyntaxHighlight::markdown(source);
        }
        if self.ensure_parsed().is_none() {
            return SyntaxHighlight::plaintext(0);
        }
        let read = self.document.read().expect("parsed document disappeared");
        let document = DocumentView::new(&*read);
        let bytes = document.bytes();
        match self.language {
            Language::Rust => SyntaxHighlight::from_scopes(rust::scopes(bytes), bytes),
            Language::Zig => SyntaxHighlight::from_scopes(zig::scopes(bytes), bytes),
            Language::Markdown => SyntaxHighlight::plaintext(bytes.len()),
        }
    }

    pub(crate) fn node_chain(&mut self, start: usize, end: usize) -> Vec<(usize, usize)> {
        if self.language == Language::Markdown {
            let Some(read) = self.document.read() else {
                return Vec::new();
            };
            let bytes = read.slice(0..read.len());
            let fences = markdown::fences(&bytes);
            return markdown::chain(
                &bytes,
                &fences,
                start.min(bytes.len()),
                end.min(bytes.len()),
            );
        }
        if self.ensure_parsed().is_none() {
            return Vec::new();
        }
        let ParserBackend::TreeSitter {
            tree: Some(tree), ..
        } = &self.backend
        else {
            return Vec::new();
        };
        let root = tree.root_node();
        let document_len = root.end_byte();
        let bytes = self
            .document
            .read()
            .map(|read| read.slice(0..read.len()).into_owned())
            .unwrap_or_default();
        let query_end = if start == end {
            start.saturating_add(1).min(document_len)
        } else {
            end
        };
        let mut node =
            root.descendant_for_byte_range(start.min(document_len), query_end.min(document_len));
        let mut result = Vec::new();
        while let Some(current) = node {
            let mut range = (current.start_byte(), current.end_byte());
            let source = &bytes[range.0.min(bytes.len())..range.1.min(bytes.len())];
            range.0 += self.language.chain_start_offset(current.kind(), source);
            if result.last().copied() != Some(range) {
                result.push(range);
            }
            node = current.parent();
        }
        result
    }
}

fn byte_point(bytes: &[u8], index: usize) -> Point {
    let index = index.min(bytes.len());
    let row = bytes[..index].iter().filter(|byte| **byte == b'\n').count();
    let column = bytes[..index]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(index, |newline| index - newline - 1);
    Point { row, column }
}

fn identifier_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    index
}
