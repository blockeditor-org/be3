use std::collections::HashMap;
use std::rc::Rc;

use be_block::ObjectId;
use beui::{Color32, Overflow, WrapAt};
use terminal_types::{
    TerminalColor, TerminalColors, TerminalCursor, TerminalLine, TerminalStyle, logical_lines,
};

#[derive(Clone, Default)]
pub struct Lines(Rc<Vec<(ObjectId, Rc<TerminalLine>)>>);

impl Lines {
    pub fn new(lines: Vec<(ObjectId, Rc<TerminalLine>)>) -> Self {
        Self(Rc::new(lines))
    }

    pub fn iter(&self) -> impl Iterator<Item = &(ObjectId, Rc<TerminalLine>)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<(ObjectId, Rc<TerminalLine>)> for Lines {
    fn from_iter<T: IntoIterator<Item = (ObjectId, Rc<TerminalLine>)>>(lines: T) -> Self {
        Self::new(lines.into_iter().collect())
    }
}

impl PartialEq for Lines {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
            || (self.0.len() == other.0.len()
                && self
                    .0
                    .iter()
                    .zip(other.0.iter())
                    .all(|((id, line), (other_id, other))| {
                        id == other_id && Rc::ptr_eq(line, other)
                    }))
    }
}

#[derive(Clone)]
pub(crate) struct Shown {
    lines: Vec<Rc<TerminalLine>>,
    pub(crate) overflow: Overflow,
    cursor: Option<(usize, usize)>,
}

impl PartialEq for Shown {
    fn eq(&self, other: &Self) -> bool {
        self.overflow == other.overflow
            && self.cursor == other.cursor
            && self.lines.len() == other.lines.len()
            && self
                .lines
                .iter()
                .zip(&other.lines)
                .all(|(line, other)| Rc::ptr_eq(line, other))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Look {
    pub(crate) color: Color32,
    pub(crate) background: Option<Color32>,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) strikethrough: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum Piece {
    Text(String, Look),
    Break,
}

impl Shown {
    pub(crate) fn history(line: Rc<TerminalLine>) -> Self {
        Self {
            lines: vec![line],
            overflow: Overflow::Wrap {
                at: WrapAt::Anywhere,
                max_lines: None,
            },
            cursor: None,
        }
    }

    pub(crate) fn pieces(&self, colors: TerminalColors) -> Vec<Piece> {
        let mut pieces = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            if index > 0 {
                pieces.push(Piece::Break);
            }
            for (text, style) in line.spans() {
                pieces.push(Piece::Text(text.to_owned(), look(style, colors)));
            }
            if let Some(padding) = self.padding(index) {
                pieces.push(Piece::Text(padding, look(TerminalStyle::default(), colors)));
            }
        }
        pieces
    }

    pub(crate) fn text(&self) -> String {
        let mut text = String::new();
        for (index, line) in self.lines.iter().enumerate() {
            text.push_str(&line.text);
            if let Some(padding) = self.padding(index) {
                text.push_str(&padding);
            }
        }
        text
    }

    pub(crate) fn caret(&self) -> Option<usize> {
        let (row, byte) = self.cursor?;
        let before: usize = self.lines[..row].iter().map(|line| line.text.len()).sum();
        Some(before + byte)
    }

    fn padding(&self, index: usize) -> Option<String> {
        let (row, byte) = self.cursor.filter(|(row, _)| *row == index)?;
        let missing = byte.checked_sub(self.lines[row].text.len())?;
        (missing > 0).then(|| " ".repeat(missing))
    }
}

pub(crate) fn screen(
    rows: &Lines,
    cursor: Option<TerminalCursor>,
) -> (Vec<ObjectId>, HashMap<ObjectId, Shown>) {
    let rows: Vec<&(ObjectId, Rc<TerminalLine>)> = rows.iter().collect();
    let mut keys = Vec::new();
    let mut shown = HashMap::new();
    for group in logical_lines(rows.iter().map(|(_, line)| line.wrapped)) {
        let key = rows[group.start].0;
        let cursor = cursor
            .map(|cursor| usize::from(cursor.row))
            .filter(|row| group.contains(row))
            .zip(cursor)
            .map(|(row, cursor)| (row - group.start, cursor.byte as usize));
        keys.push(key);
        shown.insert(
            key,
            Shown {
                lines: rows[group]
                    .iter()
                    .map(|(_, line)| Rc::clone(line))
                    .collect(),
                overflow: Overflow::Visible,
                cursor,
            },
        );
    }
    (keys, shown)
}

pub(crate) fn color(color: TerminalColor) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

fn look(style: TerminalStyle, colors: TerminalColors) -> Look {
    let mut text = style.foreground.unwrap_or(colors.foreground);
    if style.faint {
        text = mix(text, style.background.unwrap_or(colors.background));
    }
    Look {
        color: color(text),
        background: style.background.map(color),
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        strikethrough: style.strikethrough,
    }
}

fn mix(color: TerminalColor, toward: TerminalColor) -> TerminalColor {
    let half = |a: u8, b: u8| ((u16::from(a) + u16::from(b)) / 2) as u8;
    TerminalColor {
        r: half(color.r, toward.r),
        g: half(color.g, toward.g),
        b: half(color.b, toward.b),
    }
}
