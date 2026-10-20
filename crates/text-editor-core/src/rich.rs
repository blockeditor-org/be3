use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::document::{anchor_in, place_in};
use crate::{Pos, SeqOp, Sequence};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Attribute {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
    Heading,
}

impl Attribute {
    pub const INLINE: [Self; 5] = [
        Self::Bold,
        Self::Italic,
        Self::Underline,
        Self::Strikethrough,
        Self::Code,
    ];

    pub const fn is_block(self) -> bool {
        matches!(self, Self::Heading)
    }

    const fn slot(self) -> usize {
        match self {
            Self::Bold => 0,
            Self::Italic => 1,
            Self::Underline => 2,
            Self::Strikethrough => 3,
            Self::Code => 4,
            Self::Heading => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RichStyle {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
    Heading(u8),
}

impl RichStyle {
    pub const fn attribute(self) -> Attribute {
        match self {
            Self::Bold => Attribute::Bold,
            Self::Italic => Attribute::Italic,
            Self::Underline => Attribute::Underline,
            Self::Strikethrough => Attribute::Strikethrough,
            Self::Code => Attribute::Code,
            Self::Heading(_) => Attribute::Heading,
        }
    }

    pub const fn value(self) -> u8 {
        match self {
            Self::Heading(level) => level,
            _ => 1,
        }
    }

    pub const fn applies_to(self, format: TextFormat) -> bool {
        format.get(self.attribute()) == self.value()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextFormat {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub code: bool,
    pub heading: u8,
}

impl TextFormat {
    pub const fn get(self, attribute: Attribute) -> u8 {
        match attribute {
            Attribute::Bold => self.bold as u8,
            Attribute::Italic => self.italic as u8,
            Attribute::Underline => self.underline as u8,
            Attribute::Strikethrough => self.strikethrough as u8,
            Attribute::Code => self.code as u8,
            Attribute::Heading => self.heading,
        }
    }

    pub const fn with(mut self, attribute: Attribute, value: u8) -> Self {
        let on = value != 0;
        match attribute {
            Attribute::Bold => self.bold = on,
            Attribute::Italic => self.italic = on,
            Attribute::Underline => self.underline = on,
            Attribute::Strikethrough => self.strikethrough = on,
            Attribute::Code => self.code = on,
            Attribute::Heading => self.heading = value,
        }
        self
    }

    pub const fn inline(self) -> Self {
        Self { heading: 0, ..self }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormatRun {
    pub len: usize,
    pub format: TextFormat,
}

pub fn format_at(runs: &[FormatRun], index: usize) -> Option<TextFormat> {
    let mut at = 0;
    runs.iter().find_map(|run| {
        at += run.len;
        (index < at).then_some(run.format)
    })
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FormatOp {
    pub start: Pos,
    pub end: Pos,
    pub attribute: Attribute,
    pub value: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RichOp {
    Text(SeqOp<u8>),
    Format(FormatOp),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MarkId {
    pub clock: u64,
    pub client: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Mark {
    pub id: MarkId,
    pub op: FormatOp,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Marks {
    marks: Vec<Mark>,
    clock: u64,
}

impl Marks {
    pub fn loaded(text: &Sequence<u8>, formats: &[(Range<usize>, Attribute, u8)]) -> Self {
        let bytes = text.items();
        let mut marks = Self::default();
        for (range, attribute, value) in formats {
            if let Some(op) = Self::op(text, &bytes, range.clone(), *attribute, *value) {
                marks.mint(sequence::LOADED, op);
            }
        }
        marks
    }

    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    pub fn mint(&mut self, client: u64, op: FormatOp) -> Mark {
        let mark = Mark {
            id: MarkId {
                clock: self.clock + 1,
                client,
            },
            op,
        };
        self.apply(mark);
        mark
    }

    pub fn apply(&mut self, mark: Mark) -> bool {
        self.clock = self.clock.max(mark.id.clock);
        match self.marks.binary_search_by_key(&mark.id, |held| held.id) {
            Ok(_) => false,
            Err(at) => {
                self.marks.insert(at, mark);
                true
            }
        }
    }

    pub fn op(
        text: &Sequence<u8>,
        bytes: &[u8],
        range: Range<usize>,
        attribute: Attribute,
        value: u8,
    ) -> Option<FormatOp> {
        let end = range.end.min(bytes.len());
        let start = range.start.min(end);
        let (start, end) = match attribute.is_block() {
            true => (line_start(bytes, start), line_end(bytes, end)),
            false if start == end => return None,
            false => (start, end),
        };
        Some(FormatOp {
            start: anchor_in(text, start)?,
            end: anchor_in(text, end)?,
            attribute,
            value,
        })
    }

    pub fn covered(text: &Sequence<u8>, bytes: &[u8], op: &FormatOp) -> Option<Range<usize>> {
        let start = place_in(text, op.start)?;
        let end = place_in(text, op.end)?;
        match op.attribute.is_block() {
            true => (start <= end)
                .then(|| line_start(bytes, start.min(bytes.len()))..(end + 1).min(bytes.len())),
            false => (start < end).then_some(start..end),
        }
    }

    pub fn inverse(&self, text: &Sequence<u8>, bytes: &[u8], op: &FormatOp) -> Vec<FormatOp> {
        let (Some(start), Some(end)) = (place_in(text, op.start), place_in(text, op.end)) else {
            return Vec::new();
        };
        let mut values: Vec<(Range<usize>, u8)> = Vec::new();
        let mut extend = |range: Range<usize>, value: u8| match values.last_mut() {
            Some((last, held)) if *held == value && last.end + 1 >= range.start => {
                last.end = range.end;
            }
            _ => values.push((range, value)),
        };
        match op.attribute.is_block() {
            true => {
                let mut line = line_start(bytes, start.min(bytes.len()));
                loop {
                    let terminator = line_end(bytes, line);
                    if terminator > end {
                        break;
                    }
                    extend(line..terminator, self.line_heading(text, bytes, terminator));
                    if terminator == bytes.len() {
                        break;
                    }
                    line = terminator + 1;
                }
            }
            false => {
                let mut at = 0;
                for run in self.formats(text, bytes) {
                    let range = at.max(start)..(at + run.len).min(end);
                    at += run.len;
                    if range.start < range.end {
                        extend(range, run.format.get(op.attribute));
                    }
                }
            }
        }
        values
            .into_iter()
            .filter_map(|(range, value)| Self::op(text, bytes, range, op.attribute, value))
            .collect()
    }

    pub fn formats(&self, text: &Sequence<u8>, bytes: &[u8]) -> Vec<FormatRun> {
        let len = bytes.len();
        let mut inline = vec![[0u8; 5]; len + 1];
        let terminators: Vec<usize> = memchr::memchr_iter(b'\n', bytes)
            .chain(std::iter::once(len))
            .collect();
        let mut headings = vec![0u8; terminators.len()];
        for mark in &self.marks {
            let (Some(start), Some(end)) =
                (place_in(text, mark.op.start), place_in(text, mark.op.end))
            else {
                continue;
            };
            match mark.op.attribute.is_block() {
                true => {
                    let first = terminators.partition_point(|terminator| *terminator < start);
                    let last = terminators.partition_point(|terminator| *terminator <= end);
                    for heading in headings.iter_mut().take(last).skip(first) {
                        *heading = mark.op.value;
                    }
                }
                false => {
                    let slot = mark.op.attribute.slot();
                    for values in inline.iter_mut().take(end.min(len)).skip(start) {
                        values[slot] = mark.op.value;
                    }
                }
            }
        }
        let mut runs: Vec<FormatRun> = Vec::new();
        let mut line = 0;
        for (index, values) in inline.iter().enumerate() {
            let format = TextFormat {
                bold: values[0] != 0,
                italic: values[1] != 0,
                underline: values[2] != 0,
                strikethrough: values[3] != 0,
                code: values[4] != 0,
                heading: headings[line],
            };
            if terminators[line] == index {
                line += 1;
            }
            match runs.last_mut() {
                Some(last) if last.format == format => last.len += 1,
                _ => runs.push(FormatRun { len: 1, format }),
            }
        }
        runs
    }

    pub fn line_heading(&self, text: &Sequence<u8>, bytes: &[u8], index: usize) -> u8 {
        let terminator = line_end(bytes, index.min(bytes.len()));
        self.marks
            .iter()
            .rev()
            .filter(|mark| mark.op.attribute.is_block())
            .find_map(|mark| {
                let start = place_in(text, mark.op.start)?;
                let end = place_in(text, mark.op.end)?;
                (start <= terminator && terminator <= end).then_some(mark.op.value)
            })
            .unwrap_or(0)
    }
}

fn line_start(bytes: &[u8], index: usize) -> usize {
    memchr::memrchr(b'\n', &bytes[..index]).map_or(0, |newline| newline + 1)
}

fn line_end(bytes: &[u8], index: usize) -> usize {
    memchr::memchr(b'\n', &bytes[index..]).map_or(bytes.len(), |newline| index + newline)
}
