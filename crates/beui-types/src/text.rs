use std::f32::consts::{FRAC_PI_4, SQRT_2};
use std::ops::Range;

use geometry::{Rect, Vec2, pos2, vec2};

use crate::color::Color;
use crate::layout::Corners;

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
    pub color: Color,
    pub underline: bool,
    pub strikethrough: bool,
}

impl SpanStyle {
    pub fn new(font: FontId, color: impl Into<Color>) -> Self {
        Self {
            font,
            color: color.into(),
            underline: false,
            strikethrough: false,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct TextMark {
    pub range: Range<usize>,
    pub color: Color,
}

impl TextMark {
    pub fn new(range: Range<usize>, color: impl Into<Color>) -> Self {
        Self {
            range,
            color: color.into(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct TextGlyph {
    pub range: Range<usize>,
    pub glyph: String,
    pub font: FontId,
    pub color: Color,
}

impl TextGlyph {
    pub fn new(
        range: Range<usize>,
        glyph: impl Into<String>,
        font: FontId,
        color: impl Into<Color>,
    ) -> Self {
        Self {
            range,
            glyph: glyph.into(),
            font,
            color: color.into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SpanBackground {
    pub color: Color,
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
    pub color: Color,
    pub width: f32,
    pub blink: bool,
    pub flag: bool,
    pub handle: Option<CaretHandle>,
}

impl TextCaret {
    pub fn new(at: usize, color: impl Into<Color>, width: f32) -> Self {
        Self {
            at,
            color: color.into(),
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

pub const HANDLE_RADIUS: f32 = 9.0;
pub const HANDLE_GAP: f32 = 4.0;
pub const TOOLBAR_GAP: f32 = 8.0;

pub fn toolbar_anchor(carets: Rect, scale: f32) -> Rect {
    Rect::from_min_max(
        pos2(carets.min.x, carets.min.y - TOOLBAR_GAP),
        pos2(
            carets.max.x,
            carets.max.y + (HANDLE_GAP + HANDLE_RADIUS * (1.0 + SQRT_2)) * scale + TOOLBAR_GAP,
        ),
    )
}

pub fn handle_center(caret: Rect, handle: CaretHandle, scale: f32) -> Vec2 {
    let (gap, radius) = (HANDLE_GAP * scale, HANDLE_RADIUS * scale);
    let anchor = vec2(caret.min.x, caret.max.y + gap);
    match handle {
        CaretHandle::Start => anchor + vec2(-radius, radius),
        CaretHandle::End => anchor + vec2(radius, radius),
        CaretHandle::Middle => anchor + vec2(caret.width() / 2.0, radius * SQRT_2),
    }
}

pub fn handle_shape(caret: Rect, handle: CaretHandle) -> (Rect, Corners, f32) {
    let center = handle_center(caret, handle, 1.0);
    let rect = Rect::from_center_size(pos2(center.x, center.y), Vec2::splat(HANDLE_RADIUS * 2.0));
    let round = Corners::all(HANDLE_RADIUS);
    match handle {
        CaretHandle::Start => (
            rect,
            Corners {
                top_right: 0.0,
                ..round
            },
            0.0,
        ),
        CaretHandle::End => (
            rect,
            Corners {
                top_left: 0.0,
                ..round
            },
            0.0,
        ),
        CaretHandle::Middle => (
            rect,
            Corners {
                top_left: 0.0,
                ..round
            },
            FRAC_PI_4,
        ),
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Piece<'a> {
    Text {
        text: &'a str,
        style: SpanStyle,
        break_after: bool,
    },
    Inline {
        index: usize,
        size: Vec2,
        text: &'a str,
    },
    Gap(f32),
    Break,
    Icon {
        glyph: &'a str,
        style: SpanStyle,
        spacing: f32,
        text_size: f32,
    },
}

impl<'a> Piece<'a> {
    pub fn text(text: &'a str, style: SpanStyle) -> Self {
        Piece::Text {
            text,
            style,
            break_after: false,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Piece::Text { text, .. } => text.len(),
            Piece::Inline { text, .. } => text.len(),
            Piece::Gap(_) | Piece::Break | Piece::Icon { .. } => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RichOptions {
    pub wrap_width: f32,
    pub padding: (f32, f32),
    pub style: SpanStyle,
    pub hang: usize,
    pub anywhere: bool,
    pub least: bool,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct FontPatch {
    pub size: Option<f32>,
    pub family: Option<FontFamily>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
}

impl FontPatch {
    pub fn apply(self, base: FontId) -> FontId {
        FontId {
            size: self.size.unwrap_or(base.size),
            family: self.family.unwrap_or(base.family),
            bold: self.bold.unwrap_or(base.bold),
            italic: self.italic.unwrap_or(base.italic),
        }
    }
}

impl From<FontId> for FontPatch {
    fn from(font: FontId) -> Self {
        Self {
            size: Some(font.size),
            family: Some(font.family),
            bold: Some(font.bold),
            italic: Some(font.italic),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct IconSpan {
    pub scale: f32,
    pub spacing_em: f32,
}
