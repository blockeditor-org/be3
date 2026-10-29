use std::ops::Range;

use be_model::{Document, List, Model};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use ui_keys::{Key, Modifiers};

use be_block::Root;
use be_block::presence::{ChannelKind, PresenceKind};

pub const HISTORY_LINES: usize = 10_000;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Terminal {
    pub host: Option<u64>,
    pub shell: Option<TerminalShell>,
    pub history: List<TerminalLine>,
    pub screen: List<TerminalLine>,
    pub alternate: List<TerminalLine>,
    pub alternate_active: bool,
    pub cursor: TerminalCursor,
    pub size: TerminalSize,
    pub colors: TerminalColors,
    pub title: String,
}

impl Terminal {
    pub fn is_empty(&self) -> bool {
        self.history.is_empty() && self.screen.is_empty() && self.alternate.is_empty()
    }

    pub fn shown(&self) -> &List<TerminalLine> {
        match self.alternate_active {
            true => &self.alternate,
            false => &self.screen,
        }
    }
}

impl Root for Terminal {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7465_726d_696e_616c_2d62_6c6f_636b_0001);
}

pub type TerminalContent = Document<Terminal>;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct TerminalLine {
    pub text: String,
    pub runs: Vec<TerminalRun>,
    pub wrapped: bool,
}

impl TerminalLine {
    pub fn styled(text: impl Into<String>, style: TerminalStyle) -> Self {
        let mut line = Self::default();
        line.push(&text.into(), style);
        line
    }

    pub fn spans(&self) -> Vec<(&str, TerminalStyle)> {
        let mut spans = Vec::new();
        let mut at = 0;
        for run in &self.runs {
            if at >= self.text.len() {
                break;
            }
            let end = floor_char_boundary(&self.text, at + run.len as usize).max(at);
            if end > at {
                spans.push((&self.text[at..end], run.style));
            }
            at = end;
        }
        if at < self.text.len() {
            spans.push((&self.text[at..], TerminalStyle::default()));
        }
        spans
    }

    pub fn push(&mut self, text: &str, style: TerminalStyle) {
        if text.is_empty() {
            return;
        }
        let covered: usize = self.runs.iter().map(|run| run.len as usize).sum();
        if covered < self.text.len() && style != TerminalStyle::default() {
            self.runs.push(TerminalRun {
                len: (self.text.len() - covered) as u32,
                style: TerminalStyle::default(),
            });
        }
        self.text.push_str(text);
        if style == TerminalStyle::default() {
            return;
        }
        match self.runs.last_mut() {
            Some(last) if last.style == style && covered == self.text.len() - text.len() => {
                last.len += text.len() as u32;
            }
            _ => self.runs.push(TerminalRun {
                len: text.len() as u32,
                style,
            }),
        }
    }

    pub fn join(&mut self, next: &TerminalLine) {
        for (text, style) in next.spans() {
            self.push(text, style);
        }
        self.wrapped = next.wrapped;
    }

    pub fn truncate(&mut self, len: usize) {
        let len = floor_char_boundary(&self.text, len);
        self.text.truncate(len);
        let mut at = 0usize;
        self.runs.retain_mut(|run| {
            if at >= len || run.len == 0 {
                return false;
            }
            run.len = (run.len as usize).min(len - at) as u32;
            at += run.len as usize;
            true
        });
        while self
            .runs
            .last()
            .is_some_and(|run| run.style == TerminalStyle::default())
        {
            self.runs.pop();
        }
    }
}

fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

pub fn logical_lines(wrapped: impl IntoIterator<Item = bool>) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut count = 0;
    for (index, wraps) in wrapped.into_iter().enumerate() {
        count = index + 1;
        if !wraps {
            lines.push(start..count);
            start = count;
        }
    }
    if start < count {
        lines.push(start..count);
    }
    lines
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TerminalRun {
    pub len: u32,
    pub style: TerminalStyle,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(default)]
pub struct TerminalStyle {
    pub foreground: Option<TerminalColor>,
    pub background: Option<TerminalColor>,
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl TerminalStyle {
    pub const FAINT: Self = Self {
        foreground: None,
        background: None,
        bold: false,
        italic: false,
        faint: true,
        underline: false,
        strikethrough: false,
    };
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(default)]
pub struct TerminalColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TerminalColors {
    pub foreground: TerminalColor,
    pub background: TerminalColor,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TerminalShell {
    pub id: u64,
    pub running: bool,
    pub status: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum TerminalCursorStyle {
    #[default]
    Block,
    Bar,
    Underline,
    BlockHollow,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TerminalCursor {
    pub row: u16,
    pub column: u16,
    pub byte: u32,
    pub visible: bool,
    pub style: TerminalCursorStyle,
    pub blink: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalHost {
    pub machine: String,
    pub user: String,
}

impl PresenceKind for TerminalHost {
    const ID: Uuid = Uuid::from_u128(0x7465_726d_696e_616c_2d68_6f73_7400_0001);
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalKey {
    pub key: Key,
    pub modifiers: Modifiers,
    pub text: String,
    pub pressed: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalWheel {
    pub lines: i16,
    pub col: u16,
    pub row: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TerminalMessage {
    Resize(TerminalSize),
    Key(TerminalKey),
    Paste(String),
    Wheel(TerminalWheel),
}

impl ChannelKind for TerminalMessage {
    const ID: Uuid = Uuid::from_u128(0x7465_726d_696e_616c_2d63_6861_6e00_0001);
}
