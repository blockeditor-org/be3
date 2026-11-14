use std::f32::consts::{FRAC_PI_4, SQRT_2};
use std::ops::Range;

use be_lib_geometry::{Rect, Vec2, pos2, vec2};

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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextLayout {
    pub wrap_width: f32,
    pub align: TextAlign,
    pub line_spacing: f32,
    pub line_height: Option<f32>,
}

impl TextLayout {
    pub const DEFAULT: Self = Self {
        wrap_width: f32::INFINITY,
        align: TextAlign::Start,
        line_spacing: 1.0,
        line_height: None,
    };

    pub fn wrapped(wrap_width: f32) -> Self {
        Self {
            wrap_width,
            ..Self::DEFAULT
        }
    }
}

impl Default for TextLayout {
    fn default() -> Self {
        Self::DEFAULT
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
        /// Isolated for bidi, as HTML's `<bdi>`: its own direction, and a neutral to the
        /// text around it.
        isolate: bool,
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
            isolate: false,
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

/// Text as runs, for a prop that takes a string (a dialog's title) when part of it is what
/// someone typed: a run can be isolated for bidi, as HTML's `<bdi>`, so a right-to-left name
/// cannot carry the words or punctuation around it across itself. A `Text` takes a `Phrase` as
/// content and lays each run out as a `Span` (an isolated one with `isolate`); anything that
/// reads the phrase as one string (an accessibility label) joins its runs. A string converts to a
/// phrase of one plain run, so a prop of `Phrase` takes a string as before.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Phrase {
    runs: Vec<PhraseRun>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PhraseRun {
    pub text: String,
    pub isolate: bool,
}

impl Phrase {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a run in the phrase's own direction.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.push(text.into(), false);
        self
    }

    /// Adds a run isolated for bidi.
    pub fn isolated(mut self, text: impl Into<String>) -> Self {
        self.push(text.into(), true);
        self
    }

    /// Adds `text` in curly quotes, isolated inside them: the quotes belong to the words around
    /// it, so they stay where those words put them.
    pub fn quoted(self, text: impl Into<String>) -> Self {
        self.text("\u{201c}").isolated(text).text("\u{201d}")
    }

    fn push(&mut self, text: String, isolate: bool) {
        if text.is_empty() {
            return;
        }
        match self.runs.last_mut() {
            Some(last) if !isolate && !last.isolate => last.text.push_str(&text),
            _ => self.runs.push(PhraseRun { text, isolate }),
        }
    }

    pub fn runs(&self) -> &[PhraseRun] {
        &self.runs
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
}

impl std::fmt::Display for Phrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.runs.iter().try_for_each(|run| f.write_str(&run.text))
    }
}

impl From<String> for Phrase {
    fn from(text: String) -> Self {
        Self::new().text(text)
    }
}

impl From<&str> for Phrase {
    fn from(text: &str) -> Self {
        Self::new().text(text)
    }
}

beui_tree::reactive::into_prop_via!(Phrase, String, Phrase::from);

impl beui_tree::reactive::IntoProp<Phrase> for &'static str {
    fn into_prop(self) -> beui_tree::reactive::Prop<Phrase> {
        beui_tree::reactive::Prop::Static(Phrase::from(self))
    }
}
