use crate::reactive::{
    BuildsNode, ChildSegment, ChildValue, Children, ComponentContext, Get, IntoChild, IntoProp,
    IntoSegment, Memo, NodeId, Prop, ReadSignal, Ready, ScopeContext, Signal,
};
use crate::types::icons::Glyph;
use crate::types::text::{FontPatch, IconSpan};
use crate::types::{Color, FontFamily, FontId, SpanBackground};

pub enum TextChild {
    Span(Span),
    Gap(Gap),
    Break(Break),
    Item(TextItem),
}

impl ChildValue for TextChild {
    fn finish_component(&mut self, component: &ComponentContext, scope: &ScopeContext) {
        match self {
            Self::Span(value) => value.finish_component(component, scope),
            Self::Gap(value) => value.finish_component(component, scope),
            Self::Break(value) => value.finish_component(component, scope),
            Self::Item(value) => value.finish_component(component, scope),
        }
    }
}

crate::child_type!(TextChild);

pub struct SpanText(pub(crate) Prop<String>);

crate::value_child_type!(SpanText);

pub struct Span {
    pub(crate) text: Prop<String>,
    pub(crate) font: Prop<FontPatch>,
    pub(crate) color: Prop<Option<Color>>,
    pub(crate) underline: Prop<Option<bool>>,
    pub(crate) strikethrough: Prop<Option<bool>>,
    pub(crate) background: Prop<Option<SpanBackground>>,
    pub(crate) break_after: Prop<bool>,
    pub(crate) icon: Option<IconSpan>,
}

impl ChildValue for Span {}

fn font_patch(
    font: Option<FontId>,
    size: Option<f32>,
    family: Option<FontFamily>,
    bold: Option<bool>,
    italic: Option<bool>,
) -> FontPatch {
    let whole = font.map(FontPatch::from).unwrap_or_default();
    FontPatch {
        size: size.or(whole.size),
        family: family.or(whole.family),
        bold: bold.or(whole.bold),
        italic: italic.or(whole.italic),
    }
}

impl Span {
    pub(crate) fn plain(text: Prop<String>) -> Self {
        Self {
            text,
            font: Prop::Static(FontPatch::default()),
            color: Prop::Static(None),
            underline: Prop::Static(None),
            strikethrough: Prop::Static(None),
            background: Prop::Static(None),
            break_after: Prop::Static(false),
            icon: None,
        }
    }

    pub(crate) fn is_plain(&self) -> bool {
        matches!(self.font, Prop::Static(font) if font == FontPatch::default())
            && matches!(self.color, Prop::Static(None))
            && matches!(self.underline, Prop::Static(None))
            && matches!(self.strikethrough, Prop::Static(None))
            && matches!(self.background, Prop::Static(None))
            && matches!(self.break_after, Prop::Static(false))
            && self.icon.is_none()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn span(
    font: Prop<Option<FontId>>,
    size: Prop<Option<f32>>,
    family: Prop<Option<FontFamily>>,
    bold: Prop<Option<bool>>,
    italic: Prop<Option<bool>>,
    color: Prop<Option<Color>>,
    underline: Prop<Option<bool>>,
    strikethrough: Prop<Option<bool>>,
    background: Prop<Option<SpanBackground>>,
    break_after: Prop<bool>,
    children: Children<SpanText>,
) -> Span {
    let text = match children.into_only() {
        Ok(SpanText(text)) => text,
        Err(children) => {
            let run = children.into_run();
            Prop::dynamic(move || run.items().iter().map(|piece| piece.0.get()).collect())
        }
    };
    let font = match (font, size, family, bold, italic) {
        (
            Prop::Static(font),
            Prop::Static(size),
            Prop::Static(family),
            Prop::Static(bold),
            Prop::Static(italic),
        ) => Prop::Static(font_patch(font, size, family, bold, italic)),
        (font, size, family, bold, italic) => Prop::dynamic(move || {
            font_patch(
                font.get(),
                size.get(),
                family.get(),
                bold.get(),
                italic.get(),
            )
        }),
    };
    Span {
        text,
        font,
        color,
        underline,
        strikethrough,
        background,
        break_after,
        icon: None,
    }
}

pub(crate) fn icon(
    glyph: Prop<Option<Glyph>>,
    size: Prop<Option<f32>>,
    scale: f32,
    spacing_em: f32,
    color: Prop<Option<Color>>,
) -> Span {
    Span {
        font: size.map(|size| FontPatch {
            size,
            ..FontPatch::default()
        }),
        color,
        icon: Some(IconSpan { scale, spacing_em }),
        ..Span::plain(
            glyph.map(|glyph| glyph.map_or_else(String::new, |glyph| glyph.as_str().to_owned())),
        )
    }
}

macro_rules! text_child_from {
    ($($ty:ty),*) => {
        $(
            impl IntoChild<TextChild> for $ty {
                fn into_child(self) -> TextChild {
                    TextChild::Span(Span::plain(self.into_prop()))
                }
            }

            impl IntoSegment<TextChild> for $ty {
                fn into_segment(self) -> ChildSegment<TextChild> {
                    ChildSegment::One(self.into_child())
                }
            }

            impl IntoSegment<SpanText> for $ty {
                fn into_segment(self) -> ChildSegment<SpanText> {
                    ChildSegment::One(SpanText(self.into_prop()))
                }
            }
        )*
    };
}

text_child_from!(
    &'static str,
    String,
    Prop<String>,
    Memo<String>,
    Memo<Ready<String>>,
    ReadSignal<String>,
    Signal<String>
);

impl IntoChild<TextChild> for Span {
    fn into_child(self) -> TextChild {
        TextChild::Span(self)
    }
}

impl IntoSegment<TextChild> for Span {
    fn into_segment(self) -> ChildSegment<TextChild> {
        ChildSegment::One(TextChild::Span(self))
    }
}

pub struct Gap {
    pub(crate) width: f32,
}

impl ChildValue for Gap {}

impl IntoChild<TextChild> for Gap {
    fn into_child(self) -> TextChild {
        TextChild::Gap(self)
    }
}

impl IntoSegment<TextChild> for Gap {
    fn into_segment(self) -> ChildSegment<TextChild> {
        ChildSegment::One(TextChild::Gap(self))
    }
}

pub struct Break {}

impl ChildValue for Break {}

impl IntoChild<TextChild> for Break {
    fn into_child(self) -> TextChild {
        TextChild::Break(self)
    }
}

impl IntoSegment<TextChild> for Break {
    fn into_segment(self) -> ChildSegment<TextChild> {
        ChildSegment::One(TextChild::Break(self))
    }
}

pub struct TextItem {
    pub(crate) node: NodeId,
    pub(crate) data: crate::core::text::ItemData,
}

impl BuildsNode for TextItem {
    fn built_node(&self) -> NodeId {
        self.node
    }
}

impl ChildValue for TextItem {
    fn finish_component(&mut self, component: &ComponentContext, scope: &ScopeContext) {
        self.node.finish_component(component, scope);
    }
}

impl IntoChild<TextChild> for TextItem {
    fn into_child(self) -> TextChild {
        TextChild::Item(self)
    }
}

impl IntoSegment<TextChild> for TextItem {
    fn into_segment(self) -> ChildSegment<TextChild> {
        ChildSegment::One(TextChild::Item(self))
    }
}

impl From<TextItem> for Children<TextChild> {
    fn from(item: TextItem) -> Self {
        Self::from(ChildSegment::One(TextChild::Item(item)))
    }
}
