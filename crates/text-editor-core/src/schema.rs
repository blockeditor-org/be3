use crate::Language;
use crate::core::EditorCommand;
use crate::highlighter::{SynHlColorScope, SynHlFontFamily, SynHlStyle, SynHlTextSize};
use crate::rich::{Expand, MarkSet, Name};

pub mod basic;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Selected {
    pub marks: MarkSet,
    pub block: MarkSet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Break {
    Split { reset: Vec<Name> },
    Lift(Vec<Name>),
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

    fn on_break(&self, _block: &MarkSet, _empty: bool, _at_end: bool) -> Break {
        Break::Split { reset: Vec::new() }
    }

    fn lift_on_backspace(&self, _block: &MarkSet) -> Vec<Name> {
        Vec::new()
    }

    fn indent(&self, _block: &MarkSet, _outdent: bool) -> Option<(Name, Option<crate::Value>)> {
        None
    }

    fn decorate(&self, _block: &MarkSet) -> Option<Language> {
        None
    }

    fn tools(&self) -> &[Tool] {
        &[]
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
        })
    }
}
