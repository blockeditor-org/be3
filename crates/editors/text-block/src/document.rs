use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use block_editor_beui::Waker;
use block_editor_beui::be_block::be_model::{Change, Edit, ObjectId};
use block_editor_beui::be_block::block_url::{BLOCK_URL_MAX_BYTES, parse_block_urls};
use block_editor_beui::be_block::{self, TextBlock, TextContent};
use text_editor_core::{
    ChangeLog, CursorPosition, Document, DocumentEdit, DocumentRead, Pos, Sequence, TextChange,
    TextIndentation, TextLanguage, anchor_in, anchor_index_in, changed, deleted_anchor_index_in,
};
use uuid::Uuid;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum History {
    Undo,
    Redo,
}

struct State {
    text: Sequence<u8>,
    language: TextLanguage,
    indentation: TextIndentation,
    revision: u64,
    changes: ChangeLog,
    pending: TextChange,
    outgoing: Vec<Edit>,
    history: Option<History>,
    external: bool,
    client: u64,
}

impl State {
    fn bump(&mut self, change: Option<TextChange>) {
        self.revision += 1;
        self.changes.record(self.revision, change);
    }

    fn adopt(&mut self, content: &TextContent) {
        let header = header(content);
        let settings = self.language != header.0 || self.indentation != header.1;
        self.language = header.0;
        self.indentation = header.1;
        let mut text = TextBlock::body(content).cloned().unwrap_or_default();
        text.mirror();
        let change = difference(
            self.text.mirrored().unwrap_or_default(),
            text.mirrored().unwrap_or_default(),
        );
        self.text = text;
        self.external = true;
        self.bump((!settings).then_some(change));
    }

    fn apply(&mut self, edit: &Edit) -> bool {
        let body = TextBlock::BODY.index();
        let ops: Option<Vec<_>> = edit
            .0
            .iter()
            .map(|change| match change {
                Change::Text { object, field, op }
                    if *object == ObjectId::ROOT && *field == body =>
                {
                    Some(op)
                }
                _ => None,
            })
            .collect();
        let Some(ops) = ops else {
            return false;
        };
        let mut change = TextChange::NONE;
        for op in ops {
            if let Some(splices) = self.text.apply(op) {
                change = changed(change, &splices);
            }
        }
        self.external = true;
        self.bump(Some(change));
        true
    }
}

fn header(content: &TextContent) -> (TextLanguage, TextIndentation) {
    (
        editor_language(content.field(ObjectId::ROOT, TextBlock::LANGUAGE)),
        editor_indentation(content.field(ObjectId::ROOT, TextBlock::INDENTATION)),
    )
}

fn difference(old: &[u8], new: &[u8]) -> TextChange {
    let prefix = old
        .iter()
        .zip(new)
        .take_while(|(before, after)| before == after)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(before, after)| before == after)
        .count();
    TextChange {
        start: prefix,
        old_end: old.len() - suffix,
        new_end: new.len() - suffix,
    }
}

pub struct BlockDocument {
    state: RwLock<State>,
    waker: Waker,
}

impl BlockDocument {
    pub fn new(waker: Waker) -> Self {
        let mut text = Sequence::default();
        text.mirror();
        Self {
            state: RwLock::new(State {
                text,
                language: TextLanguage::default(),
                indentation: TextIndentation::default(),
                revision: 0,
                changes: ChangeLog::default(),
                pending: TextChange::NONE,
                outgoing: Vec::new(),
                history: None,
                external: false,
                client: Uuid::new_v4().as_u64_pair().0 | 1 << 63,
            }),
            waker,
        }
    }

    fn read_state(&self) -> RwLockReadGuard<'_, State> {
        self.state
            .read()
            .expect("the text document lock was poisoned")
    }

    fn write_state(&self) -> RwLockWriteGuard<'_, State> {
        self.state
            .write()
            .expect("the text document lock was poisoned")
    }

    pub fn adopt(&self, content: &TextContent) {
        self.write_state().adopt(content);
    }

    pub fn apply(&self, edit: &Edit) -> bool {
        self.write_state().apply(edit)
    }

    pub fn take_operations(&self) -> Vec<Edit> {
        std::mem::take(&mut self.write_state().outgoing)
    }

    pub fn take_history(&self) -> Option<History> {
        self.write_state().history.take()
    }

    pub fn take_external_edit(&self) -> bool {
        std::mem::take(&mut self.write_state().external)
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.read_state().text.items()
    }

    pub fn reference_ranges(&self, reference: Uuid) -> Vec<std::ops::Range<usize>> {
        let length = reference.to_string().len();
        let state = self.read_state();
        parse_block_urls(state.text.mirrored().unwrap_or_default())
            .into_iter()
            .filter(|url| url.block == reference)
            .map(|url| url.range.end - length..url.range.end)
            .collect()
    }

    fn request(&self, history: History) -> Option<Vec<CursorPosition>> {
        let mut state = self.write_state();
        state.history = Some(history);
        state.bump(Some(TextChange::NONE));
        self.waker.wake();
        None
    }
}

pub fn inside_block_url(bytes: &[u8], index: usize) -> bool {
    let start = index.saturating_sub(BLOCK_URL_MAX_BYTES);
    let end = (index + BLOCK_URL_MAX_BYTES).min(bytes.len());
    parse_block_urls(&bytes[start..end])
        .iter()
        .any(|url| url.range.start + start < index && index < url.range.end + start)
}

impl Document for BlockDocument {
    fn read(&self) -> Option<Box<dyn DocumentRead + '_>> {
        Some(Box::new(Read {
            state: self.read_state(),
        }))
    }

    fn revision(&self) -> u64 {
        self.read_state().revision
    }

    fn changes_since(&self, revision: u64) -> Option<TextChange> {
        let state = self.read_state();
        state.changes.since(revision, state.revision)
    }

    fn set_language(&self, language: TextLanguage) {
        let mut state = self.write_state();
        state.language = language;
        state.bump(None);
        state
            .outgoing
            .push(TextBlock::set_language(block_language(language)));
        self.waker.wake();
    }

    fn set_indentation(&self, indentation: TextIndentation) {
        let mut state = self.write_state();
        state.indentation = indentation;
        state.bump(None);
        state
            .outgoing
            .push(TextBlock::set_indentation(block_indentation(indentation)));
        self.waker.wake();
    }

    fn edit(&self, _cursors: Vec<CursorPosition>, edit: &mut dyn FnMut(&mut dyn DocumentEdit)) {
        let mut state = self.write_state();
        let mut transaction = Transaction {
            state: &mut state,
            changes: Vec::new(),
        };
        edit(&mut transaction);
        let changes = transaction.changes;
        if changes.is_empty() {
            return;
        }
        let change = std::mem::replace(&mut state.pending, TextChange::NONE);
        state.bump(Some(change));
        state.outgoing.push(Edit(changes));
        self.waker.wake();
    }

    fn finish_history_group(&self) {}

    fn undo(&self) -> Option<Vec<CursorPosition>> {
        self.request(History::Undo)
    }

    fn redo(&self) -> Option<Vec<CursorPosition>> {
        self.request(History::Redo)
    }
}

struct Read<'a> {
    state: RwLockReadGuard<'a, State>,
}

impl DocumentRead for Read<'_> {
    fn len(&self) -> usize {
        self.state.text.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.state.text.chunk(index)
    }

    fn anchor(&self, index: usize) -> Option<Pos> {
        anchor_in(&self.state.text, index)
    }

    fn anchor_index(&self, anchor: Pos) -> Option<usize> {
        anchor_index_in(&self.state.text, anchor)
    }

    fn deleted_anchor_index(&self, anchor: Pos) -> Option<usize> {
        deleted_anchor_index_in(&self.state.text, anchor)
    }

    fn language(&self) -> TextLanguage {
        self.state.language
    }

    fn indentation(&self) -> TextIndentation {
        self.state.indentation
    }
}

struct Transaction<'a> {
    state: &'a mut State,
    changes: Vec<Change>,
}

impl Transaction<'_> {
    fn run(&mut self, op: Option<text_editor_core::SeqOp<u8>>) {
        let Some(op) = op else {
            return;
        };
        let Some(splices) = self.state.text.apply(&op) else {
            return;
        };
        self.state.pending = changed(self.state.pending, &splices);
        self.changes.push(TextBlock::BODY.edit(ObjectId::ROOT, op));
    }
}

impl DocumentRead for Transaction<'_> {
    fn len(&self) -> usize {
        self.state.text.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.state.text.chunk(index)
    }

    fn anchor(&self, index: usize) -> Option<Pos> {
        anchor_in(&self.state.text, index)
    }

    fn anchor_index(&self, anchor: Pos) -> Option<usize> {
        anchor_index_in(&self.state.text, anchor)
    }

    fn deleted_anchor_index(&self, anchor: Pos) -> Option<usize> {
        deleted_anchor_index_in(&self.state.text, anchor)
    }

    fn language(&self) -> TextLanguage {
        self.state.language
    }

    fn indentation(&self) -> TextIndentation {
        self.state.indentation
    }
}

impl DocumentEdit for Transaction<'_> {
    fn document(&self) -> &dyn DocumentRead {
        self
    }

    fn replace(&mut self, index: usize, delete: usize, insert: &[u8]) {
        let len = self.state.text.len();
        let index = index.min(len);
        let delete = delete.min(len - index);
        if delete > 0 {
            let op = self.state.text.delete(index..index + delete);
            self.run(op);
        }
        if !insert.is_empty() {
            let client = self.state.client;
            let op = self.state.text.insert(client, index, insert.to_vec());
            self.run(op);
        }
    }

    fn replace_atomically(&mut self, index: usize, delete: usize, insert: &[u8]) {
        let len = self.state.text.len();
        let index = index.min(len);
        let delete = delete.min(len - index);
        let client = self.state.client;
        let op = self
            .state
            .text
            .replace(client, index..index + delete, insert.to_vec());
        self.run(op);
    }
}

pub const fn block_language(language: TextLanguage) -> be_block::TextLanguage {
    match language {
        TextLanguage::Markdown => be_block::TextLanguage::Markdown,
        TextLanguage::PlainText => be_block::TextLanguage::PlainText,
        TextLanguage::Rust => be_block::TextLanguage::Rust,
        TextLanguage::Zig => be_block::TextLanguage::Zig,
    }
}

pub const fn editor_language(language: be_block::TextLanguage) -> TextLanguage {
    match language {
        be_block::TextLanguage::Markdown => TextLanguage::Markdown,
        be_block::TextLanguage::PlainText => TextLanguage::PlainText,
        be_block::TextLanguage::Rust => TextLanguage::Rust,
        be_block::TextLanguage::Zig => TextLanguage::Zig,
    }
}

pub const fn block_indentation(indentation: TextIndentation) -> be_block::TextIndentation {
    match indentation {
        TextIndentation::Tabs => be_block::TextIndentation::Tabs,
        TextIndentation::Spaces { width } => be_block::TextIndentation::Spaces { width },
    }
}

pub const fn editor_indentation(indentation: be_block::TextIndentation) -> TextIndentation {
    match indentation {
        be_block::TextIndentation::Tabs => TextIndentation::Tabs,
        be_block::TextIndentation::Spaces { width } => TextIndentation::Spaces { width },
    }
}
