use crate::Language;
use crate::core::EditorCommand;
use crate::highlighter::{SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize};
use crate::rich::{Expand, Formats, MarkSet, Name, Value};

pub mod basic;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Selected {
    pub marks: MarkSet,
    pub block: MarkSet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Break {
    Split { reset: Vec<Name> },
    Lift(Vec<(Name, Option<Value>)>),
    Exit(Vec<Name>),
}

#[derive(Clone, Copy, Debug)]
pub struct Line<'a> {
    pub props: &'a MarkSet,
    pub empty: bool,
    pub at_end: bool,
    pub above: Option<(&'a MarkSet, bool)>,
    pub below: Option<&'a MarkSet>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListMarker {
    Bullet,
    Number(usize),
    Check(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableRow {
    Header,
    Delimiter,
    Body,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TableLine {
    pub row: TableRow,
    pub start: usize,
    pub last: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineLook {
    pub marker: std::ops::Range<usize>,
    pub list: Option<ListMarker>,
    pub depth: usize,
    pub continues: bool,
    pub quote: usize,
    pub code_block: bool,
    pub heading: u8,
    pub rule: bool,
    pub hang: std::ops::Range<usize>,
    pub quote_above: usize,
    pub quote_below: usize,
    pub code_above: bool,
    pub code_below: bool,
    pub table: Option<TableLine>,
}

impl LineLook {
    pub fn at(start: usize) -> Self {
        Self {
            marker: start..start,
            hang: start..start,
            ..Self::default()
        }
    }

    pub fn shifted(&self, by: isize) -> Self {
        let shift = |at: usize| at.saturating_add_signed(by);
        Self {
            marker: shift(self.marker.start)..shift(self.marker.end),
            hang: shift(self.hang.start)..shift(self.hang.end),
            table: self.table.map(|table| TableLine {
                start: shift(table.start),
                ..table
            }),
            ..self.clone()
        }
    }
}

pub fn join_neighbours<T: PartialEq>(looks: &mut [LineLook], blocks: &[Option<T>]) {
    for at in 0..looks.len() {
        let quote = looks[at].quote;
        let block = &blocks[at];
        if at > 0 {
            looks[at].quote_above = quote.min(looks[at - 1].quote);
            looks[at].code_above = block.is_some() && blocks[at - 1] == *block;
        }
        if at + 1 < looks.len() {
            looks[at].quote_below = quote.min(looks[at + 1].quote);
            looks[at].code_below = block.is_some() && blocks[at + 1] == *block;
        }
        let next = looks.get(at + 1).and_then(|below| below.table);
        if let Some(table) = &mut looks[at].table {
            table.last = next.is_none_or(|next| next.start != table.start);
        }
    }
}

#[derive(Clone, Copy)]
pub struct Tool {
    pub name: &'static str,
    pub label: &'static str,
    pub shortcut: Option<char>,
    pub active: fn(&Selected) -> bool,
    pub command: fn(&Selected) -> EditorCommand<'static>,
}

pub trait Schema: Send + Sync {
    fn expand(&self, _key: &str) -> Expand {
        Expand::After
    }

    fn leaf_style(
        &self,
        marks: &MarkSet,
        block: &MarkSet,
        decoration: Option<SynHlStyle>,
    ) -> SynHlStyle;

    fn on_break(&self, _line: &Line<'_>) -> Break {
        Break::Split { reset: Vec::new() }
    }

    fn lift_on_backspace(&self, _block: &MarkSet, _previous: Option<&MarkSet>) -> Vec<Name> {
        Vec::new()
    }

    fn indent(&self, _block: &MarkSet, _outdent: bool) -> Option<(Name, Option<Value>)> {
        None
    }

    fn decorate(&self, _block: &MarkSet) -> Option<Language> {
        None
    }

    fn tools(&self) -> &[Tool] {
        &[]
    }

    fn line_looks(&self, _formats: &Formats) -> Vec<LineLook> {
        Vec::new()
    }

    fn check(&self, _checked: bool) -> Option<(Name, Option<Value>)> {
        None
    }
}

pub struct Plain;

impl Schema for Plain {
    fn leaf_style(
        &self,
        _marks: &MarkSet,
        _block: &MarkSet,
        decoration: Option<SynHlStyle>,
    ) -> SynHlStyle {
        decoration.unwrap_or(SynHlStyle {
            color: SynHlColorScope::MarkdownPlainText,
            family: SynHlFontFamily::Proportional,
            size: SynHlTextSize::Body,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            code: false,
        })
    }
}
