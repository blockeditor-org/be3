use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use block_editor_beui::Waker;
use block_editor_beui::be_block::Edit;
use block_editor_beui::be_block::be_model::Change;
use block_editor_beui::be_block::canvas::Canvas;
use text_editor_core::{
    ChangeLog, CursorPosition, Document, DocumentEdit, DocumentRead, Pos, SeqOp, Sequence,
    TextChange, TextIndentation, TextLanguage, TextRun, anchor_in, anchor_index_in, changed,
    deleted_anchor_index_in, runs_in,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum History {
    Undo,
    Redo,
}

struct State {
    entity: Uuid,
    client: u64,
    text: Sequence<u8>,
    revision: u64,
    changes: ChangeLog,
    pending: TextChange,
    outgoing: Vec<Edit>,
    history: Option<History>,
}

impl State {
    fn bump(&mut self, change: Option<TextChange>) {
        self.revision += 1;
        self.changes.record(self.revision, change);
    }
}

pub(crate) struct LiveText {
    state: RwLock<State>,
    waker: Waker,
}

impl LiveText {
    pub(crate) fn new(entity: Uuid, client: u64, text: Sequence<u8>, waker: Waker) -> Self {
        let mut text = text;
        text.mirror();
        Self {
            state: RwLock::new(State {
                entity,
                client,
                text,
                revision: 0,
                changes: ChangeLog::default(),
                pending: TextChange::NONE,
                outgoing: Vec::new(),
                history: None,
            }),
            waker,
        }
    }

    fn read_state(&self) -> RwLockReadGuard<'_, State> {
        self.state
            .read()
            .expect("the canvas text lock was poisoned")
    }

    fn write_state(&self) -> RwLockWriteGuard<'_, State> {
        self.state
            .write()
            .expect("the canvas text lock was poisoned")
    }

    pub(crate) fn adopt(&self, text: &Sequence<u8>) -> bool {
        let mut state = self.write_state();
        if state.text == *text {
            return false;
        }
        let mut text = text.clone();
        text.mirror();
        let change = difference(
            state.text.mirrored().unwrap_or_default(),
            text.mirrored().unwrap_or_default(),
        );
        state.text = text;
        state.bump(Some(change));
        true
    }

    pub(crate) fn take_operations(&self) -> Vec<Edit> {
        std::mem::take(&mut self.write_state().outgoing)
    }

    pub(crate) fn take_history(&self) -> Option<History> {
        self.write_state().history.take()
    }

    fn request(&self, history: History) -> Option<Vec<CursorPosition>> {
        let mut state = self.write_state();
        state.history = Some(history);
        state.bump(Some(TextChange::NONE));
        self.waker.wake();
        None
    }
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

impl Document for LiveText {
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

    fn set_language(&self, _language: TextLanguage) {}

    fn set_indentation(&self, _indentation: TextIndentation) {}

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

    fn runs(&self) -> Vec<TextRun> {
        runs_in(&self.state.text)
    }

    fn language(&self) -> TextLanguage {
        TextLanguage::PlainText
    }

    fn indentation(&self) -> TextIndentation {
        TextIndentation::default()
    }
}

struct Transaction<'a> {
    state: &'a mut State,
    changes: Vec<Change>,
}

impl Transaction<'_> {
    fn run(&mut self, op: Option<SeqOp<u8>>) {
        let Some(op) = op else {
            return;
        };
        let Some(splices) = self.state.text.apply(&op) else {
            return;
        };
        self.state.pending = changed(self.state.pending, &splices);
        self.changes.push(Canvas::text_edit(self.state.entity, op));
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

    fn runs(&self) -> Vec<TextRun> {
        runs_in(&self.state.text)
    }

    fn language(&self) -> TextLanguage {
        TextLanguage::PlainText
    }

    fn indentation(&self) -> TextIndentation {
        TextIndentation::default()
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
