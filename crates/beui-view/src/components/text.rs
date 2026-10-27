pub use crate::core::text::{Break, Gap, Span, SpanText, TextChild, TextItem};
use crate::reactive::Child;
use crate::reactive::Children;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Color;
use crate::types::FontFamily;
use crate::types::FontId;
use crate::types::Overflow;
use crate::types::SpanBackground;
use crate::types::TextAlign;
use crate::types::TextCaret;
use crate::types::TextGlyph;
use crate::types::TextMark;
use crate::types::icons::Glyph;
use crate::types::text::DEFAULT_FONT_SIZE;
use beui_macros::component;
use std::rc::Rc;

#[component]
pub fn Text(
    #[prop(default = FontId::proportional(DEFAULT_FONT_SIZE))] font: Prop<FontId>,
    #[prop(default = None)] line_height: Prop<Option<f32>>,
    #[prop(default = Color::WHITE)] color: Prop<Color>,
    #[prop(default = None)] align: Prop<Option<TextAlign>>,
    #[prop(default = None)] vertical_align: Prop<Option<TextAlign>>,
    #[prop(default = Overflow::default())] overflow: Prop<Overflow>,
    #[prop(default = false)] underline: Prop<bool>,
    #[prop(default = (0.0, 0.0))] line_padding: Prop<(f32, f32)>,
    #[prop(default = 0)] hang: Prop<usize>,
    #[prop(default = Vec::new())] marks: Prop<Vec<TextMark>>,
    #[prop(default = Vec::new())] glyphs: Prop<Vec<TextGlyph>>,
    #[prop(default = Vec::new())] carets: Prop<Vec<TextCaret>>,
    children: Children<TextChild>,
) -> NodeId {
    crate::core::text(
        font,
        line_height,
        color,
        align,
        vertical_align,
        overflow,
        underline,
        line_padding,
        hang,
        marks,
        glyphs,
        carets,
        children,
    )
}

#[component]
pub fn Span(
    #[prop(default = None)] font: Prop<Option<FontId>>,
    #[prop(default = None)] size: Prop<Option<f32>>,
    #[prop(default = None)] family: Prop<Option<FontFamily>>,
    #[prop(default = None)] bold: Prop<Option<bool>>,
    #[prop(default = None)] italic: Prop<Option<bool>>,
    #[prop(default = None)] color: Prop<Option<Color>>,
    #[prop(default = None)] underline: Prop<Option<bool>>,
    #[prop(default = None)] strikethrough: Prop<Option<bool>>,
    #[prop(default = None)] background: Prop<Option<SpanBackground>>,
    #[prop(default = false)] break_after: Prop<bool>,
    children: Children<SpanText>,
) -> Span {
    crate::core::span(
        font,
        size,
        family,
        bold,
        italic,
        color,
        underline,
        strikethrough,
        background,
        break_after,
        children,
    )
}

#[component]
pub fn Icon(
    glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] size: Prop<Option<f32>>,
    #[prop(default = 1.0)] scale: f32,
    #[prop(default = 0.0)] spacing_em: f32,
    #[prop(default = None)] color: Prop<Option<Color>>,
) -> Span {
    crate::core::icon(glyph, size, scale, spacing_em, color)
}

#[component]
pub fn Gap(width: f32) -> Gap {
    crate::core::gap(width)
}

#[component]
pub fn TextItem(
    at: Option<Prop<Option<usize>>>,
    text: Option<Prop<Option<Rc<str>>>>,
    children: Option<Child>,
) -> TextItem {
    crate::core::text_item(at, text, children)
}

#[component]
pub fn Break() -> Break {
    crate::core::line_break()
}
