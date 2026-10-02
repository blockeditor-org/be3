use std::ops::Range;

use super::{
    Core, CursorPosition, Position, Selection, UndoClassification, insert_classification,
    resolve_selection,
};
use crate::document::{DocumentRead, DocumentView};

const SURROUNDING: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeCommand<'a> {
    SetComposingText(&'a str),
    CommitText(&'a str),
    FinishComposing,
    SetComposingRegion(Range<usize>),
    ReplaceText { range: Range<usize>, text: &'a str },
    DeleteSurrounding { before: usize, after: usize },
    SetSelection { anchor: usize, focus: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImeState {
    pub start: usize,
    pub text: String,
    pub selection: Range<usize>,
    pub composing: Option<Range<usize>>,
}

impl Core {
    pub fn composition(&self) -> Option<Range<usize>> {
        let (start, end) = self.composition?;
        let read = self.document.read()?;
        let start = start.resolve(&*read);
        let end = end.resolve(&*read);
        (start < end).then_some(start..end)
    }

    pub fn ime_state(&self) -> Option<ImeState> {
        let read = self.document.read()?;
        let document = DocumentView::new(&*read);
        let bytes = document.bytes();
        let cursor = self.cursor_positions.first()?;
        let selection = resolve_selection(&document, cursor.pos);
        let start = boundary(bytes, selection.left.saturating_sub(SURROUNDING));
        let end = boundary(bytes, (selection.right + SURROUNDING).min(bytes.len()));
        let text = String::from_utf8_lossy(&bytes[start..end]).into_owned();
        drop(document);
        drop(read);
        let composing = self
            .composition()
            .filter(|range| range.start >= start && range.end <= end);
        Some(ImeState {
            start,
            text,
            selection: selection.left..selection.right,
            composing,
        })
    }

    pub(super) fn ime(&mut self, command: ImeCommand<'_>) {
        if self.cursor_positions.len() > 1 {
            self.cursor_positions.truncate(1);
        }
        match command {
            ImeCommand::SetComposingText(text) => {
                let target = self.composition().unwrap_or_else(|| self.primary_range());
                let end =
                    self.replace(target.clone(), text, insert_classification(text.as_bytes()));
                self.compose(target.start..end);
            }
            ImeCommand::CommitText(text) => {
                let target = self.composition().unwrap_or_else(|| self.primary_range());
                self.composition = None;
                self.replace(target, text, insert_classification(text.as_bytes()));
            }
            ImeCommand::FinishComposing => self.composition = None,
            ImeCommand::SetComposingRegion(range) => {
                let range = self.clamp(range);
                self.compose(range);
            }
            ImeCommand::ReplaceText { range, text } => {
                let range = self.clamp(range);
                self.composition = None;
                self.replace(range, text, UndoClassification::AlwaysSplit);
            }
            ImeCommand::DeleteSurrounding { before, after } => {
                self.delete_surrounding(before, after)
            }
            ImeCommand::SetSelection { anchor, focus } => {
                let anchor = self.clamp(anchor..anchor).start;
                let focus = self.clamp(focus..focus).start;
                self.select(Selection::range(
                    self.position(anchor),
                    self.position(focus),
                ));
            }
        }
    }

    fn primary_range(&self) -> Range<usize> {
        self.cursor_positions
            .first()
            .and_then(|cursor| self.selection_range(cursor))
            .unwrap_or(0..0)
    }

    fn clamp(&self, range: Range<usize>) -> Range<usize> {
        let Some(read) = self.document.read() else {
            return 0..0;
        };
        let document = DocumentView::new(&*read);
        let bytes = document.bytes();
        let start = boundary(bytes, range.start.min(range.end).min(bytes.len()));
        let end = boundary(bytes, range.start.max(range.end).min(bytes.len()));
        start..end
    }

    fn compose(&mut self, range: Range<usize>) {
        self.composition = (range.start < range.end)
            .then(|| (self.position(range.start), self.end_position(range.end)));
    }

    fn end_position(&self, index: usize) -> Position {
        let Some(read) = self.document.read() else {
            return Position::END;
        };
        let document = DocumentView::new(&*read);
        Position {
            left: index
                .checked_sub(1)
                .and_then(|previous| document.anchor(previous)),
            right: None,
            fallback: index,
            end: false,
        }
    }

    fn replace(
        &mut self,
        range: Range<usize>,
        text: &str,
        classification: UndoClassification,
    ) -> usize {
        let end = range.start + text.len();
        if range.is_empty() && text.is_empty() {
            self.select(Selection::at(self.position(range.start)));
            return end;
        }
        let history = self.cursor_positions.clone();
        let start = self.position(range.start);
        let positions = self.apply_replacements(
            vec![(start, range.len(), text.as_bytes().to_vec())],
            classification,
            history,
        );
        let cursor = positions
            .first()
            .copied()
            .unwrap_or_else(|| self.position(end));
        self.cursor_positions = vec![CursorPosition::at(cursor)];
        end
    }

    fn delete_surrounding(&mut self, before: usize, after: usize) {
        let Some(cursor) = self.cursor_positions.first().copied() else {
            return;
        };
        let Some(read) = self.document.read() else {
            return;
        };
        let selection = resolve_selection(&DocumentView::new(&*read), cursor.pos);
        drop(read);
        let start = self
            .clamp(selection.left.saturating_sub(before)..selection.left)
            .start;
        let end = self
            .clamp(selection.right..selection.right.saturating_add(after))
            .end;
        let mut replacements = Vec::new();
        if selection.right < end {
            replacements.push((
                self.position(selection.right),
                end - selection.right,
                Vec::new(),
            ));
        }
        if start < selection.left {
            replacements.push((self.position(start), selection.left - start, Vec::new()));
        }
        if replacements.is_empty() {
            return;
        }
        let history = self.cursor_positions.clone();
        self.apply_replacements(
            replacements,
            UndoClassification::DeleteGraphemeCluster,
            history,
        );
        let right = start + (selection.right - selection.left);
        let (anchor, focus) = match selection.is_right {
            true => (start, right),
            false => (right, start),
        };
        self.select(Selection::range(
            self.position(anchor),
            self.position(focus),
        ));
    }
}

fn boundary(bytes: &[u8], mut index: usize) -> usize {
    while index > 0 && index < bytes.len() && bytes[index] & 0xc0 == 0x80 {
        index -= 1;
    }
    index
}
