use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use be_block::LiveEdit;
use be_block::be_model::{Change, Edit, ObjectId};
use block_editor_plugin::{ContentProjection, EditorHost, Projected, Waker};
use text_editor_core::{
    ChangeLog, CursorPosition, Document, DocumentEdit, DocumentRead, Pos, SeqOp, Sequence,
    TextChange, TextIndentation, TextLanguage, TextRun, anchor_in, anchor_index_in, changed,
    deleted_anchor_index_in, runs_in,
};
use uuid::Uuid;

pub trait TextField: Send + Sync + 'static {
    type Content: LiveEdit<Op = Edit> + Clone + Default + 'static;

    fn object(&self) -> ObjectId;

    fn field(&self) -> u16;

    fn text<'a>(&self, content: &'a Self::Content) -> Option<&'a Sequence<u8>>;

    fn settings(&self, _content: &Self::Content) -> (TextLanguage, TextIndentation) {
        (TextLanguage::PlainText, TextIndentation::default())
    }

    fn set_language(&self, _language: TextLanguage) -> Option<Edit> {
        None
    }

    fn set_indentation(&self, _indentation: TextIndentation) -> Option<Edit> {
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum History {
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Synced {
    Waiting,
    Unchanged,
    External,
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
}

pub struct LiveText<F: TextField> {
    state: RwLock<State>,
    field: F,
    waker: Waker,
}

impl<F: TextField> LiveText<F> {
    pub fn new(field: F, waker: Waker) -> Self {
        let mut text = Sequence::default();
        text.mirror();
        let (language, indentation) = field.settings(&F::Content::default());
        Self {
            state: RwLock::new(State {
                text,
                language,
                indentation,
                revision: 0,
                changes: ChangeLog::default(),
                pending: TextChange::NONE,
                outgoing: Vec::new(),
                history: None,
                external: false,
                client: Uuid::new_v4().as_u64_pair().0 | 1 << 63,
            }),
            field,
            waker,
        }
    }

    fn read_state(&self) -> RwLockReadGuard<'_, State> {
        self.state.read().expect("the live text lock was poisoned")
    }

    fn write_state(&self) -> RwLockWriteGuard<'_, State> {
        self.state.write().expect("the live text lock was poisoned")
    }

    pub fn adopt(&self, content: &F::Content) {
        let (language, indentation) = self.field.settings(content);
        let held = self.field.text(content);
        let mut state = self.write_state();
        let settings = state.language != language || state.indentation != indentation;
        if !settings && held.is_none_or(|held| *held == state.text) {
            return;
        }
        state.language = language;
        state.indentation = indentation;
        let mut text = held.cloned().unwrap_or_default();
        text.mirror();
        let change = TextChange::between(
            state.text.mirrored().unwrap_or_default(),
            text.mirrored().unwrap_or_default(),
        );
        state.text = text;
        state.external = true;
        state.bump((!settings).then_some(change));
    }

    pub fn apply(&self, edit: &Edit) -> bool {
        let (object, field) = (self.field.object(), self.field.field());
        let ops: Option<Vec<_>> = edit
            .0
            .iter()
            .map(|change| match change {
                Change::Text {
                    object: changed,
                    field: changed_field,
                    op,
                } if *changed == object && *changed_field == field => Some(op),
                _ => None,
            })
            .collect();
        let Some(ops) = ops else {
            return false;
        };
        let mut state = self.write_state();
        let mut change = TextChange::NONE;
        for op in ops {
            if let Some(splices) = state.text.apply(op) {
                change = changed(change, &splices);
            }
        }
        state.external = true;
        state.bump(Some(change));
        true
    }

    pub fn sync(
        &self,
        content: ContentProjection<F::Content>,
        host: &EditorHost,
        block: Uuid,
        whole: bool,
    ) -> Synced {
        let operations = self.take_operations();
        let own = !operations.is_empty();
        let mut whole = whole || (own && !host.editable());
        for operation in operations {
            content.operate(operation);
        }
        match self.take_history() {
            Some(History::Undo) => host.undo(block),
            Some(History::Redo) => host.redo(block),
            None => {}
        }
        if content.revision().is_none() {
            return Synced::Waiting;
        }
        let projected = content.take_projected();
        whole |= own
            && projected
                .iter()
                .any(|projected| matches!(projected, Projected::Applied(_)));
        if !whole {
            whole = !projected.iter().all(|projected| match projected {
                Projected::Applied(edit) => self.apply(edit),
                Projected::Rebuilt => false,
            });
        }
        if whole {
            content.read(|content| self.adopt(content));
        }
        match self.take_external_edit() {
            true => Synced::External,
            false => Synced::Unchanged,
        }
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

    pub fn with_bytes<R>(&self, read: impl FnOnce(&[u8]) -> R) -> R {
        read(self.read_state().text.mirrored().unwrap_or_default())
    }

    fn request(&self, history: History) -> Option<Vec<CursorPosition>> {
        let mut state = self.write_state();
        state.history = Some(history);
        state.bump(Some(TextChange::NONE));
        self.waker.wake();
        None
    }

    fn send(&self, state: &mut State, edit: Option<Edit>) {
        if let Some(edit) = edit {
            state.outgoing.push(edit);
            self.waker.wake();
        }
    }
}

impl<F: TextField> Document for LiveText<F> {
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
        self.send(&mut state, self.field.set_language(language));
    }

    fn set_indentation(&self, indentation: TextIndentation) {
        let mut state = self.write_state();
        state.indentation = indentation;
        state.bump(None);
        self.send(&mut state, self.field.set_indentation(indentation));
    }

    fn edit(&self, _cursors: Vec<CursorPosition>, edit: &mut dyn FnMut(&mut dyn DocumentEdit)) {
        let mut state = self.write_state();
        let mut transaction = Transaction {
            state: &mut state,
            object: self.field.object(),
            field: self.field.field(),
            changes: Vec::new(),
        };
        edit(&mut transaction);
        let changes = transaction.changes;
        if changes.is_empty() {
            return;
        }
        let change = std::mem::replace(&mut state.pending, TextChange::NONE);
        state.bump(Some(change));
        self.send(&mut state, Some(Edit(changes)));
    }

    fn finish_history_group(&self) {}

    fn undo(&self) -> Option<Vec<CursorPosition>> {
        self.request(History::Undo)
    }

    fn redo(&self) -> Option<Vec<CursorPosition>> {
        self.request(History::Redo)
    }
}

macro_rules! read_from_state {
    ($type:ty) => {
        impl DocumentRead for $type {
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
                self.state.language
            }

            fn indentation(&self) -> TextIndentation {
                self.state.indentation
            }
        }
    };
}

struct Read<'a> {
    state: RwLockReadGuard<'a, State>,
}

read_from_state!(Read<'_>);

struct Transaction<'a> {
    state: &'a mut State,
    object: ObjectId,
    field: u16,
    changes: Vec<Change>,
}

read_from_state!(Transaction<'_>);

impl Transaction<'_> {
    fn run(&mut self, op: Option<SeqOp<u8>>) {
        let Some(op) = op else {
            return;
        };
        let Some(splices) = self.state.text.apply(&op) else {
            return;
        };
        self.state.pending = changed(self.state.pending, &splices);
        self.changes.push(Change::Text {
            object: self.object,
            field: self.field,
            op,
        });
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
