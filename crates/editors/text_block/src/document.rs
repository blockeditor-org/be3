use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use block_client::{BLOCK_URL_MAX_BYTES, parse_block_urls};
use block_editor_plugin::Waker;
use block_editor_plugin::be_block::{self, TextContent, TextOp};
use similar::{Algorithm, DiffOp, capture_diff_slices};
use text_editor_core::{
    Anchor, CursorPosition, Document, DocumentEdit, DocumentRead, TextIndentation, TextLanguage,
};
use uuid::Uuid;

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct Replace {
    left: Option<Anchor>,
    right: Option<Anchor>,
    before: Vec<u8>,
    before_anchors: Vec<Anchor>,
    after: Vec<u8>,
    after_anchors: Vec<Anchor>,
}

struct Group {
    replaces: Vec<Replace>,
    cursors: Vec<CursorPosition>,
}

#[derive(Default)]
struct State {
    bytes: Vec<u8>,
    anchors: Vec<Anchor>,
    language: TextLanguage,
    indentation: TextIndentation,
    revision: u64,
    outgoing: Vec<TextOp>,
    undo: Vec<Group>,
    redo: Vec<Group>,
    group_open: bool,
    external: bool,
}

impl State {
    fn splice(&mut self, index: usize, delete: usize, bytes: &[u8], anchors: &[Anchor]) -> Replace {
        let left = index.checked_sub(1).map(|previous| self.anchors[previous]);
        let right = self.anchors.get(index + delete).copied();
        let before: Vec<u8> = self
            .bytes
            .splice(index..index + delete, bytes.iter().copied())
            .collect();
        let before_anchors: Vec<Anchor> = self
            .anchors
            .splice(index..index + delete, anchors.iter().copied())
            .collect();
        if delete > 0 {
            self.outgoing
                .push(TextOp::delete(index as u64, delete as u64));
        }
        if !bytes.is_empty() {
            self.outgoing.push(TextOp::Insert {
                at: index as u64,
                bytes: bytes.to_vec(),
            });
        }
        self.revision += 1;
        Replace {
            left,
            right,
            before,
            before_anchors,
            after: bytes.to_vec(),
            after_anchors: anchors.to_vec(),
        }
    }

    fn index_of(&self, anchor: Anchor) -> Option<usize> {
        self.anchors.iter().position(|held| *held == anchor)
    }

    fn step(&mut self, replace: &Replace, forward: bool) {
        let (expected, wanted, anchors) = match forward {
            true => (&replace.before, &replace.after, &replace.after_anchors),
            false => (&replace.after, &replace.before, &replace.before_anchors),
        };
        let start = match replace.left {
            Some(left) => match self.index_of(left) {
                Some(index) => index + 1,
                None => return,
            },
            None => 0,
        };
        let end = match replace.right {
            Some(right) => match self.index_of(right) {
                Some(index) => index,
                None => return,
            },
            None => self.bytes.len(),
        };
        if end < start || self.bytes[start..end] != expected[..] {
            return;
        }
        self.splice(start, end - start, wanted, anchors);
    }

    fn adopt(&mut self, content: &TextContent) -> bool {
        let mut changed = false;
        let language = editor_language(content.language());
        if self.language != language {
            self.language = language;
            changed = true;
        }
        let indentation = editor_indentation(content.indentation());
        if self.indentation != indentation {
            self.indentation = indentation;
            changed = true;
        }
        let incoming = content.bytes();
        if self.bytes != incoming {
            let mut anchors = Vec::with_capacity(incoming.len());
            for operation in capture_diff_slices(Algorithm::Myers, &self.bytes, incoming) {
                match operation {
                    DiffOp::Equal { old_index, len, .. } => {
                        anchors.extend_from_slice(&self.anchors[old_index..old_index + len]);
                    }
                    DiffOp::Insert { new_len, .. } | DiffOp::Replace { new_len, .. } => {
                        anchors.extend((0..new_len).map(|_| Anchor::new()));
                    }
                    DiffOp::Delete { .. } => {}
                }
            }
            self.bytes = incoming.to_vec();
            self.anchors = anchors;
            self.external = true;
            changed = true;
        }
        if changed {
            self.revision += 1;
        }
        changed
    }
}

pub struct BlockDocument {
    state: RwLock<State>,
    waker: Waker,
}

impl BlockDocument {
    pub fn new(waker: Waker) -> Self {
        Self {
            state: RwLock::new(State::default()),
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

    pub fn adopt(&self, content: &TextContent) -> bool {
        self.write_state().adopt(content)
    }

    pub fn take_operations(&self) -> Vec<TextOp> {
        std::mem::take(&mut self.write_state().outgoing)
    }

    pub fn take_external_edit(&self) -> bool {
        std::mem::take(&mut self.write_state().external)
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.read_state().bytes.clone()
    }

    pub fn reference_ranges(&self, reference: Uuid) -> Vec<std::ops::Range<usize>> {
        let length = reference.to_string().len();
        parse_block_urls(&self.read_state().bytes)
            .into_iter()
            .filter(|url| url.block == reference)
            .map(|url| url.range.end - length..url.range.end)
            .collect()
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

    fn set_language(&self, language: TextLanguage) {
        let mut state = self.write_state();
        state.language = language;
        state.revision += 1;
        state
            .outgoing
            .push(TextOp::SetLanguage(block_language(language)));
        self.waker.wake();
    }

    fn set_indentation(&self, indentation: TextIndentation) {
        let mut state = self.write_state();
        state.indentation = indentation;
        state.revision += 1;
        state
            .outgoing
            .push(TextOp::SetIndentation(block_indentation(indentation)));
        self.waker.wake();
    }

    fn edit(&self, cursors: Vec<CursorPosition>, edit: &mut dyn FnMut(&mut dyn DocumentEdit)) {
        let mut state = self.write_state();
        let mut transaction = Edit {
            state: &mut state,
            replaces: Vec::new(),
        };
        edit(&mut transaction);
        let replaces = transaction.replaces;
        if replaces.is_empty() {
            return;
        }
        state.redo.clear();
        let open = state.group_open;
        if open && let Some(group) = state.undo.last_mut() {
            group.replaces.extend(replaces);
        } else {
            state.undo.push(Group { replaces, cursors });
        }
        state.group_open = true;
        self.waker.wake();
    }

    fn finish_history_group(&self) {
        self.write_state().group_open = false;
    }

    fn undo(&self) -> Option<Vec<CursorPosition>> {
        let mut state = self.write_state();
        state.group_open = false;
        let group = state.undo.pop()?;
        for replace in group.replaces.iter().rev() {
            state.step(replace, false);
        }
        let cursors = group.cursors.clone();
        state.redo.push(group);
        self.waker.wake();
        Some(cursors)
    }

    fn redo(&self) -> Option<Vec<CursorPosition>> {
        let mut state = self.write_state();
        state.group_open = false;
        let group = state.redo.pop()?;
        for replace in &group.replaces {
            state.step(replace, true);
        }
        let cursors = group.cursors.clone();
        state.undo.push(group);
        self.waker.wake();
        Some(cursors)
    }
}

struct Read<'a> {
    state: RwLockReadGuard<'a, State>,
}

impl DocumentRead for Read<'_> {
    fn len(&self) -> usize {
        self.state.bytes.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.state.bytes.get(index..).unwrap_or_default()
    }

    fn anchor(&self, index: usize) -> Option<Anchor> {
        self.state.anchors.get(index).copied()
    }

    fn anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.state.index_of(anchor)
    }

    fn language(&self) -> TextLanguage {
        self.state.language
    }

    fn indentation(&self) -> TextIndentation {
        self.state.indentation
    }
}

struct Edit<'a> {
    state: &'a mut State,
    replaces: Vec<Replace>,
}

impl DocumentRead for Edit<'_> {
    fn len(&self) -> usize {
        self.state.bytes.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.state.bytes.get(index..).unwrap_or_default()
    }

    fn anchor(&self, index: usize) -> Option<Anchor> {
        self.state.anchors.get(index).copied()
    }

    fn anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.state.index_of(anchor)
    }

    fn language(&self) -> TextLanguage {
        self.state.language
    }

    fn indentation(&self) -> TextIndentation {
        self.state.indentation
    }
}

impl DocumentEdit for Edit<'_> {
    fn document(&self) -> &dyn DocumentRead {
        self
    }

    fn replace(&mut self, index: usize, delete: usize, insert: &[u8]) {
        let index = index.min(self.state.bytes.len());
        let delete = delete.min(self.state.bytes.len() - index);
        if delete == 0 && insert.is_empty() {
            return;
        }
        let anchors: Vec<Anchor> = insert.iter().map(|_| Anchor::new()).collect();
        let replace = self.state.splice(index, delete, insert, &anchors);
        self.replaces.push(replace);
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
