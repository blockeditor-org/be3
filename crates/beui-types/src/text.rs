use std::ops::Range;

use geometry::{Rect, Vec2};

use crate::color::Color32;

pub const DEFAULT_FONT_SIZE: f32 = 14.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum WrapAt {
    #[default]
    Words,
    Anywhere,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Overflow {
    Wrap { at: WrapAt, max_lines: Option<u32> },
    Ellipsis,
    Visible,
}

impl Overflow {
    pub fn wraps(self) -> bool {
        matches!(self, Overflow::Wrap { .. })
    }
}

impl Default for Overflow {
    fn default() -> Self {
        Overflow::Wrap {
            at: WrapAt::Words,
            max_lines: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FontFamily {
    Proportional,
    Monospace,
    Icons,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FontId {
    pub size: f32,
    pub family: FontFamily,
    pub bold: bool,
    pub italic: bool,
}

impl FontId {
    pub fn new(size: f32, family: FontFamily) -> Self {
        Self {
            size,
            family,
            bold: false,
            italic: false,
        }
    }

    pub fn proportional(size: f32) -> Self {
        Self::new(size, FontFamily::Proportional)
    }

    pub fn monospace(size: f32) -> Self {
        Self::new(size, FontFamily::Monospace)
    }

    pub fn icons(size: f32) -> Self {
        Self::new(size, FontFamily::Icons)
    }

    pub fn bold(self, bold: bool) -> Self {
        Self { bold, ..self }
    }

    pub fn italic(self, italic: bool) -> Self {
        Self { italic, ..self }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SpanStyle {
    pub font: FontId,
    pub color: Color32,
    pub underline: bool,
    pub strikethrough: bool,
}

impl SpanStyle {
    pub fn new(font: FontId, color: Color32) -> Self {
        Self {
            font,
            color,
            underline: false,
            strikethrough: false,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct TextMark {
    pub range: Range<usize>,
    pub color: Color32,
}

impl TextMark {
    pub fn new(range: Range<usize>, color: Color32) -> Self {
        Self { range, color }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct TextGlyph {
    pub range: Range<usize>,
    pub glyph: String,
    pub font: FontId,
    pub color: Color32,
}

impl TextGlyph {
    pub fn new(
        range: Range<usize>,
        glyph: impl Into<String>,
        font: FontId,
        color: Color32,
    ) -> Self {
        Self {
            range,
            glyph: glyph.into(),
            font,
            color,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SpanBackground {
    pub color: Color32,
    pub radius: f32,
    pub outset: Vec2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaretHandle {
    Start,
    End,
    Middle,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextCaret {
    pub at: usize,
    pub color: Color32,
    pub width: f32,
    pub blink: bool,
    pub flag: bool,
    pub handle: Option<CaretHandle>,
}

impl TextCaret {
    pub fn new(at: usize, color: Color32, width: f32) -> Self {
        Self {
            at,
            color,
            width,
            blink: false,
            flag: false,
            handle: None,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct LineBox {
    pub bytes: Range<usize>,
    pub rect: Rect,
    pub baseline: f32,
}
