use std::{
    borrow::Cow,
    ops::Range,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use sequence::{Pos, SeqOp, Sequence, Splice};

use crate::{ChangeLog, CursorPosition, TextChange};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextLanguage {
    #[default]
    Markdown,
    PlainText,
    Rust,
    Zig,
}

impl TextLanguage {
    pub const ALL: [Self; 4] = [Self::Markdown, Self::PlainText, Self::Rust, Self::Zig];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown",
            Self::PlainText => "Plain text",
            Self::Rust => "Rust",
            Self::Zig => "Zig",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextIndentation {
    Tabs,
    Spaces { width: u8 },
}

impl Default for TextIndentation {
    fn default() -> Self {
        Self::Spaces { width: 2 }
    }
}

impl TextIndentation {
    pub const fn byte(self) -> u8 {
        match self {
            Self::Tabs => b'\t',
            Self::Spaces { .. } => b' ',
        }
    }

    pub const fn width(self) -> u8 {
        match self {
            Self::Tabs => 1,
            Self::Spaces { width: 0 } => 1,
            Self::Spaces { width } => width,
        }
    }
}

pub trait DocumentRead {
    fn len(&self) -> usize;

    fn chunk(&self, index: usize) -> &[u8];

    fn anchor(&self, index: usize) -> Option<Pos>;

    fn anchor_index(&self, anchor: Pos) -> Option<usize>;

    fn deleted_anchor_index(&self, _anchor: Pos) -> Option<usize> {
        None
    }

    fn language(&self) -> TextLanguage;

    fn indentation(&self) -> TextIndentation;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn slice(&self, range: Range<usize>) -> Cow<'_, [u8]> {
        let end = range.end.min(self.len());
        let start = range.start.min(end);
        let chunk = self.chunk(start);
        if chunk.len() >= end - start {
            return Cow::Borrowed(&chunk[..end - start]);
        }
        let mut bytes = chunk.to_vec();
        let mut index = start + chunk.len();
        while index < end {
            let chunk = self.chunk(index);
            if chunk.is_empty() {
                break;
            }
            let taken = chunk.len().min(end - index);
            bytes.extend_from_slice(&chunk[..taken]);
            index += taken;
        }
        Cow::Owned(bytes)
    }

    fn slice_from_anchor(&self, anchor: Pos, len: usize) -> Cow<'_, [u8]> {
        match self.anchor_index(anchor) {
            Some(index) => self.slice(index..index.saturating_add(len)),
            None => Cow::Borrowed(&[]),
        }
    }
}

pub trait DocumentEdit {
    fn document(&self) -> &dyn DocumentRead;

    fn replace(&mut self, index: usize, delete: usize, insert: &[u8]);

    fn replace_atomically(&mut self, index: usize, delete: usize, insert: &[u8]);
}

pub trait Document {
    fn read(&self) -> Option<Box<dyn DocumentRead + '_>>;

    fn revision(&self) -> u64;

    fn changes_since(&self, revision: u64) -> Option<TextChange>;

    fn set_language(&self, language: TextLanguage);

    fn set_indentation(&self, indentation: TextIndentation);

    fn edit(&self, cursors: Vec<CursorPosition>, edit: &mut dyn FnMut(&mut dyn DocumentEdit));

    fn finish_history_group(&self);

    fn undo(&self) -> Option<Vec<CursorPosition>>;

    fn redo(&self) -> Option<Vec<CursorPosition>>;
}

pub struct DocumentView<'a> {
    read: &'a dyn DocumentRead,
    bytes: Cow<'a, [u8]>,
}

impl<'a> DocumentView<'a> {
    pub fn new(read: &'a dyn DocumentRead) -> Self {
        let bytes = read.slice(0..read.len());
        Self { read, bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl DocumentRead for DocumentView<'_> {
    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn chunk(&self, index: usize) -> &[u8] {
        self.bytes.get(index..).unwrap_or_default()
    }

    fn anchor(&self, index: usize) -> Option<Pos> {
        self.read.anchor(index)
    }

    fn anchor_index(&self, anchor: Pos) -> Option<usize> {
        self.read.anchor_index(anchor)
    }

    fn deleted_anchor_index(&self, anchor: Pos) -> Option<usize> {
        self.read.deleted_anchor_index(anchor)
    }

    fn language(&self) -> TextLanguage {
        self.read.language()
    }

    fn indentation(&self) -> TextIndentation {
        self.read.indentation()
    }
}

pub fn anchor_in(sequence: &Sequence<u8>, index: usize) -> Option<Pos> {
    sequence.pos(index)
}

pub fn anchor_index_in(sequence: &Sequence<u8>, anchor: Pos) -> Option<usize> {
    match sequence.place_of(anchor)? {
        (index, true) => Some(index),
        (_, false) => None,
    }
}

pub fn deleted_anchor_index_in(sequence: &Sequence<u8>, anchor: Pos) -> Option<usize> {
    match sequence.place_of(anchor)? {
        (index, false) => Some(index),
        (_, true) => None,
    }
}

pub fn changed(change: TextChange, splices: &[Splice]) -> TextChange {
    splices.iter().fold(change, |change, splice| {
        change.then(TextChange::replace(
            splice.at,
            splice.removed,
            splice.inserted,
        ))
    })
}

const LOCAL: u64 = 1;

pub struct TextBuffer {
    state: RwLock<BufferState>,
}

struct BufferState {
    text: Sequence<u8>,
    language: TextLanguage,
    indentation: TextIndentation,
    revision: u64,
    changes: ChangeLog,
    pending: TextChange,
    undo: Vec<BufferHistoryEntry>,
    redo: Vec<BufferHistoryEntry>,
    group_open: bool,
}

#[derive(Default)]
struct BufferHistoryEntry {
    undo: Vec<SeqOp<u8>>,
    redo: Vec<SeqOp<u8>>,
    cursors: Vec<CursorPosition>,
}

impl TextBuffer {
    pub fn new(bytes: impl AsRef<[u8]>) -> Self {
        let mut text = Sequence::from_items(bytes.as_ref().to_vec());
        text.mirror();
        Self {
            state: RwLock::new(BufferState {
                text,
                language: TextLanguage::default(),
                indentation: TextIndentation::default(),
                revision: 0,
                changes: ChangeLog::default(),
                pending: TextChange::NONE,
                undo: Vec::new(),
                redo: Vec::new(),
                group_open: false,
            }),
        }
    }

    fn read_state(&self) -> RwLockReadGuard<'_, BufferState> {
        self.state
            .read()
            .expect("the text buffer lock was poisoned")
    }

    fn write_state(&self) -> RwLockWriteGuard<'_, BufferState> {
        self.state
            .write()
            .expect("the text buffer lock was poisoned")
    }
}

impl BufferState {
    fn bump(&mut self, change: Option<TextChange>) {
        self.revision += 1;
        self.changes.record(self.revision, change);
    }

    fn replay(&mut self, operations: &[SeqOp<u8>]) {
        let mut change = TextChange::NONE;
        for operation in operations {
            if let Some(splices) = self.text.apply(operation) {
                change = changed(change, &splices);
            }
        }
        self.bump(Some(change));
    }
}

impl Document for TextBuffer {
    fn read(&self) -> Option<Box<dyn DocumentRead + '_>> {
        Some(Box::new(BufferRead {
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
    }

    fn set_indentation(&self, indentation: TextIndentation) {
        let mut state = self.write_state();
        state.indentation = indentation;
        state.bump(None);
    }

    fn edit(&self, cursors: Vec<CursorPosition>, edit: &mut dyn FnMut(&mut dyn DocumentEdit)) {
        let mut state = self.write_state();
        let mut transaction = BufferEdit {
            state: &mut state,
            done: BufferHistoryEntry {
                cursors,
                ..BufferHistoryEntry::default()
            },
        };
        edit(&mut transaction);
        let done = transaction.done;
        if done.redo.is_empty() {
            return;
        }
        let change = std::mem::replace(&mut state.pending, TextChange::NONE);
        state.bump(Some(change));
        state.redo.clear();
        let grouping = state.group_open;
        match state.undo.last_mut() {
            Some(open) if grouping => {
                open.undo.extend(done.undo);
                open.redo.extend(done.redo);
            }
            _ => state.undo.push(done),
        }
        state.group_open = true;
    }

    fn finish_history_group(&self) {
        self.write_state().group_open = false;
    }

    fn undo(&self) -> Option<Vec<CursorPosition>> {
        let mut state = self.write_state();
        state.group_open = false;
        let entry = state.undo.pop()?;
        let undo: Vec<SeqOp<u8>> = entry.undo.iter().rev().cloned().collect();
        state.replay(&undo);
        let cursors = entry.cursors.clone();
        state.redo.push(entry);
        Some(cursors)
    }

    fn redo(&self) -> Option<Vec<CursorPosition>> {
        let mut state = self.write_state();
        state.group_open = false;
        let entry = state.redo.pop()?;
        state.replay(&entry.redo);
        let cursors = entry.cursors.clone();
        state.undo.push(entry);
        Some(cursors)
    }
}

struct BufferRead<'a> {
    state: RwLockReadGuard<'a, BufferState>,
}

impl DocumentRead for BufferRead<'_> {
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

struct BufferEdit<'a> {
    state: &'a mut BufferState,
    done: BufferHistoryEntry,
}

impl BufferEdit<'_> {
    fn run(&mut self, operation: Option<SeqOp<u8>>) {
        let Some(operation) = operation else {
            return;
        };
        let Some((undo, redo)) = self.state.text.inverse(&operation) else {
            return;
        };
        let Some(splices) = self.state.text.apply(&operation) else {
            return;
        };
        self.state.pending = changed(self.state.pending, &splices);
        self.done.undo.push(undo);
        self.done.redo.push(redo);
    }
}

impl DocumentRead for BufferEdit<'_> {
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

impl DocumentEdit for BufferEdit<'_> {
    fn document(&self) -> &dyn DocumentRead {
        self
    }

    fn replace(&mut self, index: usize, delete: usize, insert: &[u8]) {
        let len = self.state.text.len();
        let index = index.min(len);
        let delete = delete.min(len - index);
        if delete > 0 {
            let operation = self.state.text.delete(index..index + delete);
            self.run(operation);
        }
        if !insert.is_empty() {
            let operation = self.state.text.insert(LOCAL, index, insert.to_vec());
            self.run(operation);
        }
    }

    fn replace_atomically(&mut self, index: usize, delete: usize, insert: &[u8]) {
        let len = self.state.text.len();
        let index = index.min(len);
        let delete = delete.min(len - index);
        let operation = self
            .state
            .text
            .replace(LOCAL, index..index + delete, insert.to_vec());
        self.run(operation);
    }
}
