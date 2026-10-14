use std::rc::Rc;

use crate::reactive::{
    Child, ChildSegment, ChildValue, Children, FromPlaced, IntoChild, IntoProp,
    IntoSegment, IntoSlotHost, Memo, Placed, Prop, ReadSignal, SlotChild, SlotHost, SlotId,
    create_effect, with_document,
};
use beui_core::base::text::{SpanContent, SpanHandle, TextNode, TextPart};
use beui_core::color::Color32;
use beui_core::font::FontId;
use beui_core::node::{NodeId, NodeOf};
use beui_core::rich::{TextCaret, TextMark};

use beui_core::base::text::DEFAULT_FONT_SIZE;
use beui_core::font::TextAlign;
use beui_macros::component;

#[component]
pub fn Text(
    #[prop(default = String::new())] string: Prop<String>,
    #[prop(default = DEFAULT_FONT_SIZE)] font_size: Prop<f32>,
    line_height: Option<Prop<f32>>,
    #[prop(default = Color32::WHITE)] color: Prop<Color32>,
    align: Option<Prop<TextAlign>>,
    vertical_align: Option<Prop<TextAlign>>,
    #[prop(default = false)] wrap: Prop<bool>,
    #[prop(default = false)] monospace: Prop<bool>,
    #[prop(default = false)] bold: Prop<bool>,
    #[prop(default = false)] italic: Prop<bool>,
    #[prop(default = false)] icon: Prop<bool>,
    #[prop(default = false)] clip: Prop<bool>,
    #[prop(default = false)] underline: Prop<bool>,
    #[prop(default = false)] ellipsis: Prop<bool>,
    #[prop(default = (0.0, 0.0))] line_padding: Prop<(f32, f32)>,
    #[prop(default = Vec::new())] marks: Prop<Vec<TextMark>>,
    #[prop(default = Vec::new())] carets: Prop<Vec<TextCaret>>,
    children: Children<TextChild>,
) -> NodeId {
    let vertical_default = match align {
        Some(_) => TextAlign::Center,
        None => TextAlign::Start,
    };
    let align = align.unwrap_or(Prop::Static(TextAlign::Start));
    let vertical_align = vertical_align.unwrap_or(Prop::Static(vertical_default));
    let node = with_document(|document| {
        document.create_text(String::new(), DEFAULT_FONT_SIZE, Color32::WHITE)
    });
    create_effect(move || {
        with_document(|document| document.set_text_horizontal_align(node, align.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_text_vertical_align(node, vertical_align.get()))
    });
    create_effect(move || with_document(|document| document.set_text_wrap(node, wrap.get())));
    create_effect(move || {
        with_document(|document| document.set_text_monospace(node, monospace.get()))
    });
    create_effect(move || with_document(|document| document.set_text_bold(node, bold.get())));
    create_effect(move || with_document(|document| document.set_text_italic(node, italic.get())));
    create_effect(move || with_document(|document| document.set_text_icon(node, icon.get())));
    create_effect(move || with_document(|document| document.set_text_clip(node, clip.get())));
    create_effect(move || {
        with_document(|document| document.set_text_underline(node, underline.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_text_ellipsis(node, ellipsis.get()))
    });
    create_effect(move || with_document(|document| document.set_text(node, string.get())));
    create_effect(move || {
        with_document(|document| document.set_text_font_size(node, font_size.get()))
    });
    if let Some(line_height) = line_height {
        create_effect(move || {
            with_document(|document| document.set_text_line_height(node, Some(line_height.get())))
        });
    }
    create_effect(move || with_document(|document| document.set_text_color(node, color.get())));
    create_effect(move || {
        with_document(|document| document.set_text_line_padding(node, line_padding.get()))
    });
    create_effect(move || with_document(|document| document.set_text_marks(node, marks.get())));
    create_effect(move || with_document(|document| document.set_text_carets(node, carets.get())));
    children.mount(node);
    node.id()
}

pub enum TextChild {
    Span(Span),
    Item(TextItem),
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
        }
    }
}

struct TextHost(NodeOf<TextNode>);

impl SlotHost<TextChild> for TextHost {
    fn store(&self, child: TextChild) -> TextPart {
        match child {
            TextChild::Span(span) => TextPart::Span(span.bind(self.0)),
            TextChild::Item(item) => item.store(),
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
    ReadSignal<String>
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
    font: Option<Prop<FontId>>,
    color: Option<Prop<Color32>>,
    underline: Option<Prop<bool>>,
    strikethrough: Option<Prop<bool>>,
    break_after: Prop<bool>,
}

impl ChildValue for Span {}

fn bind<T: Clone + 'static>(
    text: NodeOf<TextNode>,
    handle: &SpanHandle,
    prop: Option<Prop<T>>,
    write: fn(&mut SpanContent, T),
) {
    let Some(Prop::Dynamic(read)) = prop else {
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

impl Span {
    fn plain(text: Prop<String>) -> Self {
        Self {
            text,
            font: None,
            color: None,
            underline: None,
            strikethrough: None,
            break_after: Prop::Static(false),
        }
    }

    fn peek(&self) -> SpanContent {
        SpanContent {
            text: self.text.peek(),
            font: self.font.as_ref().map(Prop::peek),
            color: self.color.as_ref().map(Prop::peek),
            underline: self.underline.as_ref().map(Prop::peek),
            strikethrough: self.strikethrough.as_ref().map(Prop::peek),
            break_after: self.break_after.peek(),
        }
    }

    fn bind(self, text: NodeOf<TextNode>) -> SpanHandle {
        let handle = SpanHandle::new(self.peek());
        bind(text, &handle, Some(self.text), |content, value| {
            content.text = value;
        });
        bind(text, &handle, self.font, |content, value| {
            content.font = Some(value);
        });
        bind(text, &handle, self.color, |content, value| {
            content.color = Some(value);
        });
        bind(text, &handle, self.underline, |content, value| {
            content.underline = Some(value);
        });
        bind(text, &handle, self.strikethrough, |content, value| {
            content.strikethrough = Some(value);
        });
        bind(text, &handle, Some(self.break_after), |content, value| {
            content.break_after = value;
        });
        handle
    }
}

#[component]
pub fn Span(
    text: Option<Prop<String>>,
    font: Option<Prop<FontId>>,
    color: Option<Prop<Color32>>,
    underline: Option<Prop<bool>>,
    strikethrough: Option<Prop<bool>>,
    #[prop(default = false)] break_after: Prop<bool>,
    children: Children<SpanText>,
) -> Span {
    let text = text.unwrap_or_else(|| {
        let run = children.into_run();
        Prop::dynamic(move || run.items().iter().map(|piece| piece.0.get()).collect())
    });
    Span {
        text,
        font,
        color,
        underline,
        strikethrough,
        break_after,
    }
}

pub type TextItem = Placed<TextNode>;

#[component]
pub fn TextItem(at: Option<Prop<Option<usize>>>, children: Option<Child>) -> TextItem {
    let item = with_document(|document| {
        let item = document.create_text_item();
        if let Some(child) = children {
            document.set_text_item_child(item, child);
        }
        item
    });
    let anchored = at.is_some();
    if let Some(at) = at {
        create_effect(move || with_document(|document| document.set_text_item_at(item, at.get())));
    }
    TextItem::new(item.id(), anchored)
}
