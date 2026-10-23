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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineLook {
    pub marker: std::ops::Range<usize>,
    pub list: Option<ListMarker>,
    pub depth: usize,
    pub continues: bool,
    pub quote: usize,
    pub code_block: bool,
}

impl LineLook {
    pub fn at(start: usize) -> Self {
        Self {
            marker: start..start,
            ..Self::default()
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
