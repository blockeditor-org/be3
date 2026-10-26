use std::rc::Rc;

use crate::reactive::{
    Child, ChildSegment, ChildValue, Children, FromPlaced, Get, IntoChild, IntoProp, IntoSegment,
    IntoSlotHost, Memo, Placed, Prop, ReadSignal, Ready, Signal, SlotChild, SlotHost, SlotId,
    create_binding, create_effect, with_document,
};
use beui_core::base::text::{
    FontPatch, IconSpan, SpanContent, SpanHandle, TextNode, TextPart, TextStyle,
};
use beui_core::color::Color32;
use beui_core::font::{FontFamily, FontId};
use beui_core::icons::Glyph;
use beui_core::node::{NodeId, NodeOf};
use beui_core::rich::{SpanBackground, TextCaret, TextGlyph, TextMark};

use beui_core::base::text::DEFAULT_FONT_SIZE;
use beui_core::font::{Overflow, TextAlign};
use beui_macros::component;

#[component]
pub fn Text(
    #[prop(default = FontId::proportional(DEFAULT_FONT_SIZE))] font: Prop<FontId>,
    #[prop(default = None)] line_height: Prop<Option<f32>>,
    #[prop(default = Color32::WHITE)] color: Prop<Color32>,
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
    let (string, children) = match children.into_only() {
        Ok(TextChild::Span(span)) if span.is_plain() => (span.text, Children::default()),
        Ok(child) => (
            Prop::Static(String::new()),
            Children::from(ChildSegment::One(child)),
        ),
        Err(children) => (Prop::Static(String::new()), children),
    };
    let (initial, string) = match string {
        Prop::Static(initial) => (initial, None),
        string => (String::new(), Some(string)),
    };
    let node =
        with_document(|document| document.create_text(initial, DEFAULT_FONT_SIZE, Color32::WHITE));
    create_binding(move || {
        let horizontal = align.get();
        let style = TextStyle {
            font: font.get(),
            line_height: line_height.get(),
            color: color.get(),
            horizontal: horizontal.unwrap_or(TextAlign::Start),
            vertical: vertical_align.get().unwrap_or(match horizontal {
                Some(_) => TextAlign::Center,
                None => TextAlign::Start,
            }),
            overflow: overflow.get(),
            underline: underline.get(),
            padding: line_padding.get(),
            hang: hang.get(),
        };
        with_document(|document| document.set_text_style(node, style));
    });
    if let Some(string) = string {
        create_binding(move || {
            let string = string.get();
            with_document(|document| document.set_text(node, string))
        });
    }
    create_binding(move || {
        let marks = marks.get();
        with_document(|document| document.set_text_marks(node, marks))
    });
    create_binding(move || {
        let glyphs = glyphs.get();
        with_document(|document| document.set_text_glyphs(node, glyphs))
    });
    create_binding(move || {
        let carets = carets.get();
        with_document(|document| document.set_text_carets(node, carets))
    });
    children.mount(node);
    node.id()
}

pub enum TextChild {
    Span(Span),
    Gap(Gap),
    Break(Break),
    Item(TextItem),
}

impl ChildValue for TextChild {
    fn finish_component(
        &mut self,
        component: &crate::reactive::ComponentContext,
        scope: &crate::reactive::ScopeContext,
    ) {
        match self {
            Self::Span(value) => value.finish_component(component, scope),
            Self::Gap(value) => value.finish_component(component, scope),
            Self::Break(value) => value.finish_component(component, scope),
            Self::Item(value) => value.finish_component(component, scope),
        }
    }
}

crate::child_type!(TextChild);

impl SlotChild for TextChild {
    type Stored = TextPart;

    fn store(self) -> TextPart {
        match self {
            TextChild::Span(_) => panic!(
                "a `Span` is kept by the `Text` it is written in, which binds its props; \
                 it cannot be kept in a run of children away from one"
            ),
            TextChild::Item(item) => item.store(),
            TextChild::Gap(gap) => TextPart::Gap(gap.width),
            TextChild::Break(_) => TextPart::Break,
        }
    }
}

struct TextHost(NodeOf<TextNode>);

impl SlotHost<TextChild> for TextHost {
    fn store(&self, child: TextChild) -> TextPart {
        match child {
            TextChild::Span(span) => TextPart::Span(span.bind(self.0)),
            TextChild::Item(item) => item.store(),
            TextChild::Gap(gap) => TextPart::Gap(gap.width),
            TextChild::Break(_) => TextPart::Break,
        }
    }

    fn append(&self, stored: TextPart) {
        with_document(|document| document.append_child_item(self.0, stored));
    }

    fn open(&self) -> SlotId {
        with_document(|document| document.open_child_slot(self.0))
    }

    fn fill(&self, slot: SlotId, items: Vec<TextPart>) {
        with_document(|document| {
            if document.contains(self.0) {
                document.fill_child_slot(self.0, slot, items);
            }
        });
    }
}

impl IntoSlotHost<TextChild> for NodeOf<TextNode> {
    fn into_slot_host(self) -> Rc<dyn SlotHost<TextChild>> {
        Rc::new(TextHost(self))
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

impl FromPlaced<TextNode> for TextChild {
    fn from_placed(child: TextItem) -> Self {
        TextChild::Item(child)
    }
}

pub struct SpanText(Prop<String>);

crate::value_child_type!(SpanText);

pub struct Span {
    text: Prop<String>,
    font: Prop<FontPatch>,
    color: Prop<Option<Color32>>,
    underline: Prop<Option<bool>>,
    strikethrough: Prop<Option<bool>>,
    background: Prop<Option<SpanBackground>>,
    break_after: Prop<bool>,
    icon: Option<IconSpan>,
}

impl ChildValue for Span {}

fn bind<T: Clone + 'static>(
    text: NodeOf<TextNode>,
    handle: &SpanHandle,
    prop: Prop<T>,
    write: fn(&mut SpanContent, T),
) {
    let Prop::Dynamic(read) = prop else {
        return;
    };
    let handle = handle.clone();
    create_effect(move || {
        let value = read.get();
        with_document(|document| {
            document.update_text_span(text, &handle, |content| write(content, value))
        });
    });
}

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
    fn plain(text: Prop<String>) -> Self {
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

    fn is_plain(&self) -> bool {
        matches!(self.font, Prop::Static(font) if font == FontPatch::default())
            && matches!(self.color, Prop::Static(None))
            && matches!(self.underline, Prop::Static(None))
            && matches!(self.strikethrough, Prop::Static(None))
            && matches!(self.background, Prop::Static(None))
            && matches!(self.break_after, Prop::Static(false))
            && self.icon.is_none()
    }

    fn get_untracked(&self) -> SpanContent {
        SpanContent {
            text: self.text.get_untracked(),
            font: self.font.get_untracked(),
            color: self.color.get_untracked(),
            underline: self.underline.get_untracked(),
            strikethrough: self.strikethrough.get_untracked(),
            background: self.background.get_untracked(),
            break_after: self.break_after.get_untracked(),
            icon: self.icon,
        }
    }

    fn bind(self, text: NodeOf<TextNode>) -> SpanHandle {
        let handle = SpanHandle::new(self.get_untracked());
        bind(text, &handle, self.text, |content, value| {
            content.text = value;
        });
        bind(text, &handle, self.font, |content, value| {
            content.font = value;
        });
        bind(text, &handle, self.color, |content, value| {
            content.color = value;
        });
        bind(text, &handle, self.underline, |content, value| {
            content.underline = value;
        });
        bind(text, &handle, self.strikethrough, |content, value| {
            content.strikethrough = value;
        });
        bind(text, &handle, self.background, |content, value| {
            content.background = value;
        });
        bind(text, &handle, self.break_after, |content, value| {
            content.break_after = value;
        });
        handle
    }
}

#[component]
pub fn Span(
    #[prop(default = None)] font: Prop<Option<FontId>>,
    #[prop(default = None)] size: Prop<Option<f32>>,
    #[prop(default = None)] family: Prop<Option<FontFamily>>,
    #[prop(default = None)] bold: Prop<Option<bool>>,
    #[prop(default = None)] italic: Prop<Option<bool>>,
    #[prop(default = None)] color: Prop<Option<Color32>>,
    #[prop(default = None)] underline: Prop<Option<bool>>,
    #[prop(default = None)] strikethrough: Prop<Option<bool>>,
    #[prop(default = None)] background: Prop<Option<SpanBackground>>,
    #[prop(default = false)] break_after: Prop<bool>,
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

#[component]
pub fn Icon(
    glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] size: Prop<Option<f32>>,
    #[prop(default = 1.0)] scale: f32,
    #[prop(default = 0.0)] spacing_em: f32,
    #[prop(default = None)] color: Prop<Option<Color32>>,
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

pub struct Gap {
    width: f32,
}

impl ChildValue for Gap {}

#[component]
pub fn Gap(width: f32) -> Gap {
    Gap { width }
}

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

pub type TextItem = Placed<TextNode>;

#[component]
pub fn TextItem(
    at: Option<Prop<Option<usize>>>,
    text: Option<Prop<Option<Rc<str>>>>,
    children: Option<Child>,
) -> TextItem {
    let item = with_document(|document| {
        let item = document.create_text_item();
        if let Some(child) = children {
            document.set_text_item_child(item, child);
        }
        item
    });
    if let Some(text) = text {
        create_effect(move || {
            let text = text.get();
            with_document(|document| document.set_text_item_text(item, text))
        });
    }
    let anchored = at.is_some();
    if let Some(at) = at {
        create_effect(move || {
            let at = at.get();
            with_document(|document| document.set_text_item_at(item, at))
        });
    }
    TextItem::new(item.id(), anchored)
}

pub struct Break {}

impl ChildValue for Break {}

#[component]
pub fn Break() -> Break {
    Break {}
}

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
