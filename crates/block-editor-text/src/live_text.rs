use std::marker::PhantomData;
use std::ops::Range;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use block_editor_beui::be_block::Root;
use block_editor_beui::be_block::be_model::marks::Rich;
use block_editor_beui::be_block::be_model::{
    Change, Document as Content, Edit, FieldRef, ObjectId, RichText, Text, Value,
};
use block_editor_beui::block_editor_plugin::{ContentProjection, EditorHost, Projected, Waker};
use text_editor_core::{
    ChangeLog, CursorPosition, Document, DocumentEdit, DocumentRead, Formats, Pos, SeqOp,
    TextChange, TextIndentation, TextLanguage, TextRun, anchor_in, anchor_index_in, changed,
    deleted_anchor_index_in, format_change, format_op, formats_in, runs_in,
};
use uuid::Uuid;

#[cfg(test)]
mod tests;

pub trait TextField: Send + Sync + 'static {
    type Root: Root;

    fn object(&self) -> ObjectId;

    fn field(&self) -> u16;

    fn settings(&self, _content: &Content<Self::Root>) -> Option<(TextLanguage, TextIndentation)> {
        None
    }

    fn set_language(&self, _language: TextLanguage) -> Option<Edit> {
        None
    }

    fn set_indentation(&self, _indentation: TextIndentation) -> Option<Edit> {
        None
    }
}

pub struct FieldOf<R> {
    object: ObjectId,
    field: u16,
    root: PhantomData<fn() -> R>,
}

impl<R> FieldOf<R> {
    pub fn rich<M>(object: ObjectId, field: FieldRef<M, RichText>) -> Self {
        Self {
            object,
            field: field.index(),
            root: PhantomData,
        }
    }

    pub fn plain<M>(object: ObjectId, field: FieldRef<M, Text>) -> Self {
        Self {
            object,
            field: field.index(),
            root: PhantomData,
        }
    }
}

impl<R: Root> TextField for FieldOf<R> {
    type Root = R;

    fn object(&self) -> ObjectId {
        self.object
    }

    fn field(&self) -> u16 {
        self.field
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
    rich: Rich,
    formatted: bool,
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

    fn bytes(&self) -> &[u8] {
        self.rich.text.mirrored().unwrap_or_default()
    }
}

pub struct LiveText<F: TextField> {
    state: RwLock<State>,
    field: F,
    waker: Waker,
}

impl<F: TextField> LiveText<F> {
    pub fn new(field: F, waker: Waker) -> Self {
        let mut rich = Rich::default();
        rich.text.mirror();
        Self {
            state: RwLock::new(State {
                rich,
                formatted: false,
                language: TextLanguage::PlainText,
                indentation: TextIndentation::default(),
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

    pub fn adopt(&self, content: &Content<F::Root>) {
        let held = content
            .tree()
            .object(self.field.object())
            .and_then(|object| object.fields().get(usize::from(self.field.field())));
        let (mut rich, formatted) = match held {
            Some(Value::Rich(rich)) => (rich.clone(), true),
            Some(Value::Text(text)) => (
                Rich {
                    text: text.clone(),
                    ..Rich::default()
                },
                false,
            ),
            _ => (Rich::default(), false),
        };
        let (language, indentation) = self.field.settings(content).unwrap_or(match formatted {
            true => (TextLanguage::RichText, TextIndentation::default()),
            false => (TextLanguage::PlainText, TextIndentation::default()),
        });
        let mut state = self.write_state();
        let settings = state.language != language
            || state.indentation != indentation
            || state.formatted != formatted;
        let formats = state.rich.marks != rich.marks;
        if !settings && !formats && state.rich.text == rich.text {
            return;
        }
        rich.text.mirror();
        let change = TextChange::between(state.bytes(), rich.text.mirrored().unwrap_or_default());
        state.rich = rich;
        state.formatted = formatted;
        state.language = language;
        state.indentation = indentation;
        state.external = true;
        state.bump((!settings && !formats).then_some(change));
    }

    pub fn apply(&self, edit: &Edit) -> bool {
        let (object, field) = (self.field.object(), self.field.field());
        let ours = |changed: &ObjectId, into: &u16| *changed == object && *into == field;
        let mine = edit.0.iter().all(|change| match change {
            Change::Text {
                object: changed,
                field: into,
                ..
            } => ours(changed, into),
            Change::Mark {
                object: changed,
                field: into,
                ..
            }
            | Change::Unmark {
                object: changed,
                field: into,
                ..
            } => ours(changed, into) && self.read_state().formatted,
            _ => false,
        });
        if !mine {
            return false;
        }
        let mut state = self.write_state();
        let mut change = TextChange::NONE;
        for applied in &edit.0 {
            match applied {
                Change::Text { op, .. } => {
                    if let Some(splices) = state.rich.apply(op) {
                        change = changed(change, &splices);
                    }
                }
                Change::Mark { mark, .. } => {
                    if state.rich.mark(mark.clone()) {
                        change = change.then(format_change(&state.rich.text, &mark.op));
                    }
                }
                Change::Unmark { id, .. } => {
                    if let Some(op) = state.rich.unmark(*id) {
                        change = change.then(format_change(&state.rich.text, &op));
                    }
                }
                _ => {}
            }
        }
        state.external = true;
        state.bump(Some(change));
        true
    }

    pub fn sync(
        &self,
        content: &ContentProjection<Content<F::Root>>,
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
        if content.revision().is_err() {
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
            let _ = content.read(|content| self.adopt(content));
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
        self.read_state().bytes().to_vec()
    }

    pub fn with_bytes<T>(&self, read: impl FnOnce(&[u8]) -> T) -> T {
        read(self.read_state().bytes())
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
                self.state.rich.text.len()
            }

            fn chunk(&self, index: usize) -> &[u8] {
                self.state.rich.text.chunk(index)
            }

            fn anchor(&self, index: usize) -> Option<Pos> {
                anchor_in(&self.state.rich.text, index)
            }

            fn anchor_index(&self, anchor: Pos) -> Option<usize> {
                anchor_index_in(&self.state.rich.text, anchor)
            }

            fn deleted_anchor_index(&self, anchor: Pos) -> Option<usize> {
                deleted_anchor_index_in(&self.state.rich.text, anchor)
            }

            fn runs(&self) -> Vec<TextRun> {
                runs_in(&self.state.rich.text)
            }

            fn formats(&self) -> Option<Arc<Formats>> {
                formats_in(
                    &self.state.rich.text,
                    &self.state.rich.marks,
                    self.state.language,
                )
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
        let Some(splices) = self.state.rich.apply(&op) else {
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
        let len = self.state.rich.text.len();
        let index = index.min(len);
        let delete = delete.min(len - index);
        if delete > 0 {
            let op = self.state.rich.text.delete(index..index + delete);
            self.run(op);
        }
        if !insert.is_empty() {
            let client = self.state.client;
            let op = self.state.rich.text.insert(client, index, insert.to_vec());
            self.run(op);
        }
    }

    fn replace_atomically(&mut self, index: usize, delete: usize, insert: &[u8]) {
        let len = self.state.rich.text.len();
        let index = index.min(len);
        let delete = delete.min(len - index);
        let client = self.state.client;
        let op = self
            .state
            .rich
            .text
            .replace(client, index..index + delete, insert.to_vec());
        self.run(op);
    }

    fn format(&mut self, range: Range<usize>, change: &text_editor_core::Change) {
        let state = &mut *self.state;
        if !state.formatted || state.language != TextLanguage::RichText {
            return;
        }
        let bytes = state.rich.text.items();
        let Some(op) = format_op(&state.rich.text, &state.rich.marks, &bytes, range, change) else {
            return;
        };
        state.pending = state.pending.then(format_change(&state.rich.text, &op));
        let mark = state.rich.marks.mint(&state.rich.text, state.client, op);
        self.changes.push(Change::Mark {
            object: self.object,
            field: self.field,
            mark,
        });
    }
}
