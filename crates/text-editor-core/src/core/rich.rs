use std::ops::Range;
use std::sync::Arc;

use super::{Core, CursorPosition, Position, UndoClassification, resolve_selection};
use crate::document::{DocumentEdit, DocumentRead, DocumentView};
use crate::rich::{
    Change, Expand, Formats, MarkSet, Name, Scope, VOID, Value, line_end, line_start,
};
use crate::schema::{Break, Plain, Schema, Selected};

pub struct Transform<'a> {
    edit: &'a mut dyn DocumentEdit,
    schema: &'a dyn Schema,
    selections: Vec<Range<usize>>,
}

impl Transform<'_> {
    pub fn selections(&self) -> &[Range<usize>] {
        &self.selections
    }

    pub fn document(&self) -> &dyn DocumentRead {
        self.edit.document()
    }

    pub fn formats(&self) -> Option<Formats> {
        self.edit.document().formats()
    }

    pub fn replace(&mut self, range: Range<usize>, text: &[u8]) {
        self.edit.replace(range.start, range.len(), text);
    }

    pub fn set_mark(&mut self, range: Range<usize>, key: &Name, value: Option<Value>) {
        let expand = self.schema.expand(key.as_str());
        self.edit.format(range, &inline(key, expand, value));
    }

    pub fn set_block(&mut self, range: Range<usize>, key: &Name, value: Option<Value>) {
        self.edit.format(range, &block(key, value));
    }
}

fn inline(key: &Name, expand: Expand, value: Option<Value>) -> Change {
    Change {
        scope: Scope::Inline,
        expand,
        key: key.clone(),
        value,
    }
}

fn block(key: &Name, value: Option<Value>) -> Change {
    Change {
        scope: Scope::Block,
        expand: Expand::After,
        key: key.clone(),
        value,
    }
}

fn inherited(
    schema: &dyn Schema,
    bytes: &[u8],
    formats: &Formats,
    index: usize,
    len: usize,
) -> MarkSet {
    let next = index + len;
    let paragraph_start = index == 0 || bytes.get(index - 1) == Some(&b'\n');
    let after = formats.marks_at(next).filter(|_| bytes.get(next).is_some());
    let marks = match bytes.get(next) {
        Some(byte) if paragraph_start && *byte != b'\n' => after,
        _ => index
            .checked_sub(1)
            .and_then(|before| formats.marks_at(before)),
    };
    let Some(marks) = marks else {
        return MarkSet::default();
    };
    MarkSet::new(
        marks
            .iter()
            .filter(|(key, value)| {
                schema.expand(key.as_str()) == Expand::After
                    || after.is_some_and(|after| after.has(key.as_str(), value))
            })
            .map(|(key, value)| (key.clone(), value.clone())),
    )
}

pub(super) fn conform_inserted(
    edit: &mut dyn DocumentEdit,
    schema: &dyn Schema,
    index: usize,
    insert: &[u8],
    typed: Option<MarkSet>,
) {
    if insert.is_empty() {
        return;
    }
    let Some(formats) = edit.document().formats() else {
        return;
    };
    let bytes = edit.document().slice(0..edit.document().len()).into_owned();
    let end = index + insert.len();
    let wanted = typed.unwrap_or_else(|| inherited(schema, &bytes, &formats, index, insert.len()));
    let mut keys: Vec<Name> = wanted.iter().map(|(key, _)| key.clone()).collect();
    for (_, marks) in formats.marks_in(index..end) {
        keys.extend(marks.iter().map(|(key, _)| key.clone()));
    }
    keys.sort();
    keys.dedup();
    for key in keys {
        let value = wanted.get(key.as_str());
        let differs = formats
            .marks_in(index..end)
            .any(|(_, marks)| marks.get(key.as_str()) != value);
        if differs {
            let expand = schema.expand(key.as_str());
            edit.format(index..end, &inline(&key, expand, value.cloned()));
        }
    }
}

impl Core {
    pub fn set_schema(&mut self, schema: Arc<dyn Schema>) {
        self.schema = Some(schema);
        self.highlighter_language = None;
        self.sync_highlighter();
    }

    pub fn schema(&self) -> Arc<dyn Schema> {
        self.schema.clone().unwrap_or_else(|| Arc::new(Plain))
    }

    pub(super) fn forget_stale_typing(&mut self) {
        let Some((position, _)) = &self.typing else {
            return;
        };
        let Some(read) = self.document.read() else {
            return;
        };
        let at = position.resolve(&*read);
        let kept = match self.cursor_positions.as_slice() {
            [cursor] => {
                cursor.pos.anchor.resolve(&*read) == at && cursor.pos.focus.resolve(&*read) == at
            }
            _ => false,
        };
        drop(read);
        if !kept {
            self.typing = None;
        }
    }

    pub fn selected(&self) -> Option<Selected> {
        let read = self.document.read()?;
        let formats = read.formats()?;
        let document = DocumentView::new(&*read);
        let bytes = document.bytes();
        let schema = self.schema();
        let cursor = self.cursor_positions.first()?;
        let range = resolve_selection(&document, cursor.pos);
        let block = shared_block(&formats, range.left..range.right);
        if range.left == range.right
            && let Some((position, marks)) = &self.typing
            && position.resolve(&document) == range.left
        {
            return Some(Selected {
                marks: marks.clone(),
                block,
            });
        }
        let mut shown = formats
            .marks_in(range.left..range.right)
            .filter(|(within, _)| bytes[within.clone()].iter().any(|byte| *byte != b'\n'))
            .map(|(_, marks)| marks.clone());
        let marks = match shown.next() {
            Some(first) => shown.fold(first, |shared, marks| shared.shared(&marks)),
            None => inherited(&*schema, bytes, &formats, range.left, 0),
        };
        Some(Selected { marks, block })
    }

    fn ranges(&self, blocks: bool) -> Vec<(Position, Position)> {
        let Some(read) = self.document.read() else {
            return Vec::new();
        };
        let document = DocumentView::new(&*read);
        self.cursor_positions
            .iter()
            .map(|cursor| resolve_selection(&document, cursor.pos))
            .filter(|range| blocks || range.left < range.right)
            .map(|range| {
                (
                    Position::at(&document, range.left),
                    Position::at(&document, range.right),
                )
            })
            .collect()
    }

    fn rich(&self) -> bool {
        self.document
            .read()
            .is_some_and(|read| read.formats().is_some())
    }

    fn edit_rich(&mut self, edit: &mut dyn FnMut(&mut dyn DocumentEdit, &dyn Schema)) {
        let schema = self.schema();
        self.prepare_history_group(UndoClassification::AlwaysSplit);
        self.document
            .edit(self.cursor_positions.clone(), &mut |transaction| {
                edit(transaction, &*schema);
            });
        self.document.finish_history_group();
    }

    pub(super) fn set_mark(&mut self, key: &Name, value: Option<Value>) {
        let Some(selected) = self.selected() else {
            return;
        };
        let ranges = self.ranges(false);
        if ranges.is_empty() {
            let Some(caret) = self.cursor_positions.first().map(|cursor| cursor.pos.focus) else {
                return;
            };
            let caret = match self.document.read() {
                Some(read) => Position::at(&*read, caret.resolve(&*read)),
                None => return,
            };
            self.typing = Some((caret, selected.marks.with(key, value)));
            return;
        }
        self.typing = None;
        self.edit_rich(&mut |edit, schema| {
            let expand = schema.expand(key.as_str());
            for (start, end) in &ranges {
                let range = start.resolve(edit.document())..end.resolve(edit.document());
                edit.format(range, &inline(key, expand, value.clone()));
            }
        });
    }

    pub(super) fn set_block(&mut self, at: Option<Position>, key: &Name, value: Option<Value>) {
        if !self.rich() {
            return;
        }
        let ranges = match at {
            Some(at) => vec![(at, at)],
            None => self.ranges(true),
        };
        self.edit_rich(&mut |edit, _| {
            for (start, end) in &ranges {
                let range = start.resolve(edit.document())..end.resolve(edit.document());
                edit.format(range, &block(key, value.clone()));
            }
        });
    }

    pub(super) fn insert_void(&mut self, marks: &MarkSet) {
        if !self.rich() {
            return;
        }
        let ranges = self.ranges(true);
        let typing = self.typing.take().map(|(_, marks)| marks);
        let mut carets = Vec::new();
        self.edit_rich(&mut |edit, schema| {
            carets.clear();
            for (start, end) in &ranges {
                let start = start.resolve(edit.document());
                let end = end.resolve(edit.document());
                edit.replace(start, end - start, VOID.as_bytes());
                conform_inserted(edit, schema, start, VOID.as_bytes(), typing.clone());
                for (key, value) in marks.iter() {
                    let expand = schema.expand(key.as_str());
                    edit.format(
                        start..start + VOID.len(),
                        &inline(key, expand, Some(value.clone())),
                    );
                }
                carets.push(Position::at(edit.document(), start + VOID.len()));
            }
        });
        for (cursor, caret) in self.cursor_positions.iter_mut().zip(carets) {
            *cursor = CursorPosition::at(caret);
        }
    }

    pub(super) fn transform(&mut self, transform: fn(&mut Transform<'_>)) {
        if !self.rich() {
            return;
        }
        let selections: Vec<Range<usize>> = self
            .ranges(true)
            .into_iter()
            .filter_map(|(start, end)| {
                let read = self.document.read()?;
                Some(start.resolve(&*read)..end.resolve(&*read))
            })
            .collect();
        self.typing = None;
        self.edit_rich(&mut |edit, schema| {
            transform(&mut Transform {
                edit,
                schema,
                selections: selections.clone(),
            });
        });
    }

    pub(super) fn break_block(&mut self) -> bool {
        let Some(read) = self.document.read() else {
            return false;
        };
        let Some(formats) = read.formats() else {
            return false;
        };
        let document = DocumentView::new(&*read);
        let bytes = document.bytes();
        let schema = self.schema();
        let plans: Vec<(Position, usize, Break)> = self
            .cursor_positions
            .iter()
            .map(|cursor| {
                let range = resolve_selection(&document, cursor.pos);
                let start = line_start(bytes, range.left);
                let end = line_end(bytes, range.right);
                let props = formats
                    .block_at(range.left)
                    .map(|block| block.props.clone())
                    .unwrap_or_default();
                let empty = start == end;
                let at_end = range.right == end;
                (
                    Position::at(&document, range.left),
                    range.right - range.left,
                    schema.on_break(&props, empty, at_end),
                )
            })
            .collect();
        drop(document);
        drop(read);
        let typing = self.typing.take();
        let mut carets = Vec::new();
        self.edit_rich(&mut |edit, schema| {
            carets.clear();
            for (position, removed, plan) in &plans {
                let at = position.resolve(edit.document());
                match plan {
                    Break::Lift(keys) => {
                        for key in keys {
                            edit.format(at..at, &block(key, None));
                        }
                        carets.push(Position::at(edit.document(), at));
                    }
                    Break::Split { reset } => {
                        let typed = typing
                            .as_ref()
                            .filter(|(caret, _)| caret.resolve(edit.document()) == at)
                            .map(|(_, marks)| marks.clone());
                        edit.replace(at, *removed, b"\n");
                        conform_inserted(edit, schema, at, b"\n", typed);
                        let next = at + 1;
                        let props = edit
                            .document()
                            .formats()
                            .and_then(|formats| {
                                formats.block_at(next).map(|block| block.props.clone())
                            })
                            .unwrap_or_default();
                        for key in reset.iter().filter(|key| props.get(key.as_str()).is_some()) {
                            edit.format(next..next, &block(key, None));
                        }
                        carets.push(Position::at(edit.document(), next));
                    }
                }
            }
        });
        for (cursor, caret) in self.cursor_positions.iter_mut().zip(carets) {
            *cursor = CursorPosition::at(caret);
        }
        true
    }

    pub(super) fn lift_at_block_start(&mut self) -> bool {
        let [cursor] = self.cursor_positions.as_slice() else {
            return false;
        };
        let Some(read) = self.document.read() else {
            return false;
        };
        let Some(formats) = read.formats() else {
            return false;
        };
        let document = DocumentView::new(&*read);
        let range = resolve_selection(&document, cursor.pos);
        if range.left != range.right || line_start(document.bytes(), range.left) != range.left {
            return false;
        }
        let props = formats
            .block_at(range.left)
            .map(|block| block.props.clone())
            .unwrap_or_default();
        let keys = self.schema().lift_on_backspace(&props);
        if keys.is_empty() {
            return false;
        }
        let at = Position::at(&document, range.left);
        drop(document);
        drop(read);
        self.edit_rich(&mut |edit, _| {
            let at = at.resolve(edit.document());
            for key in &keys {
                edit.format(at..at, &block(key, None));
            }
        });
        true
    }

    pub(super) fn indent_blocks(&mut self, outdent: bool) -> bool {
        let Some(read) = self.document.read() else {
            return false;
        };
        let Some(formats) = read.formats() else {
            return false;
        };
        let document = DocumentView::new(&*read);
        let schema = self.schema();
        let changes: Vec<(Position, Name, Option<Value>)> = self
            .cursor_positions
            .iter()
            .flat_map(|cursor| {
                let range = resolve_selection(&document, cursor.pos);
                formats
                    .blocks
                    .iter()
                    .filter(move |block| block.end >= range.left && block.start <= range.right)
                    .collect::<Vec<_>>()
            })
            .filter_map(|held| {
                let (key, value) = schema.indent(&held.props, outdent)?;
                Some((Position::at(&document, held.start), key, value))
            })
            .collect();
        drop(document);
        drop(read);
        if !changes.is_empty() {
            self.edit_rich(&mut |edit, _| {
                for (at, key, value) in &changes {
                    let at = at.resolve(edit.document());
                    edit.format(at..at, &block(key, value.clone()));
                }
            });
        }
        true
    }
}

fn shared_block(formats: &Formats, range: Range<usize>) -> MarkSet {
    let mut touched = formats
        .blocks
        .iter()
        .filter(|block| block.end >= range.start && block.start <= range.end)
        .map(|block| block.props.clone());
    match touched.next() {
        Some(first) => touched.fold(first, |shared, props| shared.shared(&props)),
        None => MarkSet::default(),
    }
}
