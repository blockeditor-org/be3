use std::collections::VecDeque;

use be_block::LiveEdit;
use be_block::be_model::{Anchor, Change, Field, FieldRef, List};
use be_block::{Edit, ObjectId};
use terminal_types::{
    HISTORY_LINES, Terminal, TerminalContent, TerminalLine, TerminalShell, TerminalStyle,
};

use terminal_emulator::Shown;

const LARGEST_EDIT: usize = 256 * 1024;

type Rows = FieldRef<Terminal, List<TerminalLine>>;

struct Tail {
    id: ObjectId,
    line: TerminalLine,
    written: TerminalLine,
}

struct History<'a> {
    document: &'a TerminalContent,
    ids: VecDeque<ObjectId>,
    tail: Option<Tail>,
}

impl<'a> History<'a> {
    fn new(document: &'a TerminalContent) -> Self {
        Self {
            document,
            ids: document.ids(ObjectId::ROOT, Terminal::HISTORY).into(),
            tail: None,
        }
    }

    fn load_tail(&mut self) {
        if self.tail.is_some() {
            return;
        }
        self.tail = self.ids.back().map(|id| {
            let line = self.document.read::<TerminalLine>(*id).unwrap_or_default();
            Tail {
                id: *id,
                written: line.clone(),
                line,
            }
        });
    }

    fn flush(&mut self, changes: &mut Vec<Change>) {
        if let Some(tail) = self.tail.take() {
            set_line(changes, tail.id, &tail.written, &tail.line);
        }
    }

    fn append(
        &mut self,
        changes: &mut Vec<Change>,
        line: TerminalLine,
        row: Option<(ObjectId, TerminalLine)>,
    ) {
        self.load_tail();
        if let Some(tail) = &mut self.tail
            && tail.line.wrapped
        {
            tail.line.join(&line);
            if let Some((id, _)) = row {
                changes.push(Change::remove(id));
            }
            return;
        }
        self.flush(changes);
        let (id, written) = match row {
            Some((id, written)) => {
                changes.push(Terminal::HISTORY.move_into(ObjectId::ROOT, Anchor::End, id));
                (id, written)
            }
            None => {
                let (id, change) = Terminal::HISTORY.insert(ObjectId::ROOT, Anchor::End, &line);
                changes.push(change);
                (id, line.clone())
            }
        };
        self.ids.push_back(id);
        self.tail = Some(Tail { id, line, written });
    }

    fn retract(&mut self, changes: &mut Vec<Change>, rows: &[TerminalLine]) {
        for row in rows.iter().rev() {
            self.load_tail();
            let Some(tail) = &mut self.tail else {
                return;
            };
            let keep = tail.line.text.len().saturating_sub(row.text.len());
            if keep == 0 {
                changes.push(Change::remove(tail.id));
                self.ids.pop_back();
                self.tail = None;
                continue;
            }
            tail.line.truncate(keep);
            tail.line.wrapped = true;
        }
    }

    fn close(&mut self) {
        self.load_tail();
        if let Some(tail) = &mut self.tail {
            tail.line.wrapped = false;
        }
    }

    fn finish(mut self, changes: &mut Vec<Change>) {
        self.flush(changes);
        while self.ids.len() > HISTORY_LINES {
            if let Some(id) = self.ids.pop_front() {
                changes.push(Change::remove(id));
            }
        }
    }
}

fn set_line(changes: &mut Vec<Change>, id: ObjectId, old: &TerminalLine, new: &TerminalLine) {
    if old.text != new.text {
        changes.push(TerminalLine::TEXT.set(id, &new.text));
    }
    if old.runs != new.runs {
        changes.push(TerminalLine::RUNS.set(id, &new.runs));
    }
    if old.wrapped != new.wrapped {
        changes.push(TerminalLine::WRAPPED.set(id, &new.wrapped));
    }
}

fn set<F: Field + PartialEq + be_block::be_model::Register>(
    document: &TerminalContent,
    changes: &mut Vec<Change>,
    field: FieldRef<Terminal, F>,
    value: &F,
) {
    if document.field(ObjectId::ROOT, field) != *value {
        changes.push(field.set(ObjectId::ROOT, value));
    }
}

fn rows(document: &TerminalContent, field: Rows) -> Vec<(ObjectId, TerminalLine)> {
    document
        .ids(ObjectId::ROOT, field)
        .into_iter()
        .map(|id| (id, document.read::<TerminalLine>(id).unwrap_or_default()))
        .collect()
}

fn replace_rows(
    changes: &mut Vec<Change>,
    field: Rows,
    existing: &[(ObjectId, TerminalLine)],
    wanted: &[TerminalLine],
) {
    for index in 0..existing.len().max(wanted.len()) {
        match (existing.get(index), wanted.get(index)) {
            (Some((id, old)), Some(new)) => set_line(changes, *id, old, new),
            (Some((id, _)), None) => changes.push(Change::remove(*id)),
            (None, Some(new)) => changes.push(field.insert(ObjectId::ROOT, Anchor::End, new).1),
            (None, None) => {}
        }
    }
}

pub(crate) fn sync(
    document: &TerminalContent,
    returned: &[TerminalLine],
    scrolled: Vec<TerminalLine>,
    shown: &Shown,
    shell: Option<TerminalShell>,
) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut history = History::new(document);
    history.retract(&mut changes, returned);
    let mut screen: VecDeque<(ObjectId, TerminalLine)> = rows(document, Terminal::SCREEN).into();
    for line in scrolled {
        let row = screen.pop_front();
        history.append(&mut changes, line, row);
    }
    let screen: Vec<_> = screen.into();
    let alternate = rows(document, Terminal::ALTERNATE);
    match shown.alternate {
        true => replace_rows(&mut changes, Terminal::ALTERNATE, &alternate, &shown.rows),
        false => {
            replace_rows(&mut changes, Terminal::SCREEN, &screen, &shown.rows);
            replace_rows(&mut changes, Terminal::ALTERNATE, &alternate, &[]);
        }
    }
    set(
        document,
        &mut changes,
        Terminal::ALTERNATE_ACTIVE,
        &shown.alternate,
    );
    set(document, &mut changes, Terminal::CURSOR, &shown.cursor);
    set(document, &mut changes, Terminal::SIZE, &shown.size);
    set(document, &mut changes, Terminal::COLORS, &shown.colors);
    set(document, &mut changes, Terminal::TITLE, &shown.title);
    set(document, &mut changes, Terminal::SHELL, &shell);
    history.finish(&mut changes);
    changes
}

pub(crate) fn retire(document: &TerminalContent, marker: Option<String>) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut history = History::new(document);
    let mut screen = rows(document, Terminal::SCREEN);
    while let Some((id, line)) = screen.last()
        && line.runs.is_empty()
        && line.text.trim_end().is_empty()
    {
        changes.push(Change::remove(*id));
        screen.pop();
    }
    for (id, line) in screen {
        history.append(&mut changes, line.clone(), Some((id, line)));
    }
    history.close();
    let alternate = rows(document, Terminal::ALTERNATE);
    replace_rows(&mut changes, Terminal::ALTERNATE, &alternate, &[]);
    set(document, &mut changes, Terminal::ALTERNATE_ACTIVE, &false);
    if let Some(marker) = marker {
        history.append(
            &mut changes,
            TerminalLine::styled(marker, TerminalStyle::FAINT),
            None,
        );
    }
    history.finish(&mut changes);
    changes
}

pub(crate) fn edits(changes: Vec<Change>) -> Vec<Edit> {
    let mut edits = Vec::new();
    let mut current = Vec::new();
    let mut size = 0;
    for change in changes {
        let length = TerminalContent::encode_operation(&Edit::from(change.clone())).len();
        if size + length > LARGEST_EDIT && !current.is_empty() {
            edits.push(Edit(std::mem::take(&mut current)));
            size = 0;
        }
        size += length;
        current.push(change);
    }
    if !current.is_empty() {
        edits.push(Edit(current));
    }
    edits
}
