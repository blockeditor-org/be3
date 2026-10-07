use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

use block_editor_beui::Waker;
use block_editor_beui::be_block::block_url::{BLOCK_URL_MAX_BYTES, parse_block_urls};
use block_editor_beui::be_block::{self, TextContent, TextOp};
use similar::{Algorithm, DiffOp, capture_diff_slices};
use text_editor_core::{
    Anchor, AnchorTable, ChangeLog, CursorPosition, Document, DocumentEdit, DocumentRead,
    TextChange, TextIndentation, TextLanguage,
};
use uuid::Uuid;

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct Replace {
    left: Option<Anchor>,
    right: Option<Anchor>,
    before: Vec<u8>,
    before_anchors: Vec<(usize, Anchor)>,
    after: Vec<u8>,
    after_anchors: Vec<(usize, Anchor)>,
}

struct Group {
    replaces: Vec<Replace>,
    cursors: Vec<CursorPosition>,
}

#[derive(Default)]
struct State {
    bytes: Vec<u8>,
    anchors: Mutex<AnchorTable>,
    language: TextLanguage,
    indentation: TextIndentation,
    revision: u64,
    changes: ChangeLog,
    outgoing: Vec<TextOp>,
    undo: Vec<Group>,
    redo: Vec<Group>,
    group_open: bool,
    external: bool,
}

impl State {
    fn anchors(&self) -> MutexGuard<'_, AnchorTable> {
        self.anchors
            .lock()
            .expect("the text document's anchors were poisoned")
    }

    fn anchor(&self, index: usize) -> Option<Anchor> {
        (index < self.bytes.len()).then(|| self.anchors().anchor(index))
    }

    fn bump(&mut self, change: Option<TextChange>) {
        self.revision += 1;
        self.changes.record(self.revision, change);
    }

    fn splice(&mut self, index: usize, delete: usize, bytes: &[u8]) -> Replace {
        let left = index
            .checked_sub(1)
            .and_then(|previous| self.anchor(previous));
        let right = self.anchor(index + delete);
        let before: Vec<u8> = self
            .bytes
            .splice(index..index + delete, bytes.iter().copied())
            .collect();
        let before_anchors = self.anchors().splice(index, delete, bytes.len());
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
        self.bump(Some(TextChange::replace(index, delete, bytes.len())));
        Replace {
            left,
            right,
            before,
            before_anchors,
            after: bytes.to_vec(),
            after_anchors: Vec::new(),
        }
    }

    fn index_of(&self, anchor: Anchor) -> Option<usize> {
        self.anchors().index(anchor)
    }

    fn step(&mut self, replace: &mut Replace, forward: bool) {
        let (expected, wanted, restored) = match forward {
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
        let wanted = wanted.clone();
        let restored = restored.clone();
        let stepped = self.splice(start, end - start, &wanted);
        self.anchors().restore(start, &restored);
        match forward {
            true => replace.before_anchors = stepped.before_anchors,
            false => replace.after_anchors = stepped.before_anchors,
        }
    }

    fn adopt(&mut self, content: &TextContent) -> bool {
        let mut changed = false;
        let mut settings = false;
        let language = editor_language(content.language());
        if self.language != language {
            self.language = language;
            settings = true;
        }
        let indentation = editor_indentation(content.indentation());
        if self.indentation != indentation {
            self.indentation = indentation;
            settings = true;
        }
        changed |= settings;
        let incoming = content.bytes();
        let mut change = Some(TextChange::NONE);
        if self.bytes != incoming {
            let operations = capture_diff_slices(Algorithm::Myers, &self.bytes, incoming);
            let unequal = |operation: &&DiffOp| !matches!(operation, DiffOp::Equal { .. });
            if let (Some(first), Some(last)) = (
                operations.iter().find(unequal),
                operations.iter().rev().find(unequal),
            ) {
                change = Some(TextChange {
                    start: first.old_range().start,
                    old_end: last.old_range().end,
                    new_end: last.new_range().end,
                });
            }
            let kept: Vec<(usize, usize, usize)> = operations
                .into_iter()
                .filter_map(|operation| match operation {
                    DiffOp::Equal {
                        old_index,
                        new_index,
                        len,
                    } => Some((old_index, new_index, len)),
                    _ => None,
                })
                .collect();
            self.anchors().remap(|index| {
                let run = kept.partition_point(|(old, _, len)| old + len <= index);
                match kept.get(run) {
                    Some((old, new, _)) if *old <= index => Ok(new + (index - old)),
                    Some((_, new, _)) => Err(*new),
                    None => Err(incoming.len()),
                }
            });
            self.bytes = incoming.to_vec();
            self.external = true;
            changed = true;
        }
        if changed {
            self.bump(change.filter(|_| !settings));
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
            .push(TextOp::SetLanguage(block_language(language)));
        self.waker.wake();
    }

    fn set_indentation(&self, indentation: TextIndentation) {
        let mut state = self.write_state();
        state.indentation = indentation;
        state.bump(None);
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
        let mut group = state.undo.pop()?;
        for replace in group.replaces.iter_mut().rev() {
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
        let mut group = state.redo.pop()?;
        for replace in &mut group.replaces {
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
        self.state.anchor(index)
    }

    fn anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.state.index_of(anchor)
    }

    fn deleted_anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.state.anchors().deleted_at(anchor)
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
        self.state.anchor(index)
    }

    fn anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.state.index_of(anchor)
    }

    fn deleted_anchor_index(&self, anchor: Anchor) -> Option<usize> {
        self.state.anchors().deleted_at(anchor)
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
        let replace = self.state.splice(index, delete, insert);
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
