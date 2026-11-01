use crate::reactive::Child;
use crate::reactive::ChildSegment;
use crate::reactive::Children;
use crate::reactive::Get;
use crate::reactive::IntoSlotHost;
use crate::reactive::Prop;
use crate::reactive::SlotChild;
use crate::reactive::SlotHost;
use crate::reactive::SlotId;
use crate::reactive::create_binding;
use crate::reactive::create_effect;
use crate::text_children::{Span, TextChild, TextItem};
use beui_core::base::text::SpanContent;
use beui_core::base::text::SpanHandle;
use beui_core::base::text::TextNode;
use beui_core::base::text::TextPart;
use beui_core::base::text::TextStyle;
use beui_core::current::with_document;
use beui_core::node::NodeOf;
use beui_core::tree::ParentKind;
use beui_types::Color;
use beui_types::DEFAULT_FONT_SIZE;
use beui_types::FontId;
use beui_types::NodeId;
use beui_types::Overflow;
use beui_types::TextAlign;
use beui_types::TextCaret;
use beui_types::TextGlyph;
use beui_types::TextMark;
use std::rc::Rc;

pub(crate) fn text(
    font: Prop<FontId>,
    line_height: Prop<Option<f32>>,
    color: Prop<Color>,
    align: Prop<Option<TextAlign>>,
    vertical_align: Prop<Option<TextAlign>>,
    overflow: Prop<Overflow>,
    underline: Prop<bool>,
    line_padding: Prop<(f32, f32)>,
    hang: Prop<usize>,
    marks: Prop<Vec<TextMark>>,
    glyphs: Prop<Vec<TextGlyph>>,
    carets: Prop<Vec<TextCaret>>,
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
        with_document(|document| document.create_text(initial, DEFAULT_FONT_SIZE, Color::WHITE));
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

impl SlotChild for TextChild {
    type Stored = TextPart;

    fn store(self) -> TextPart {
        match self {
            TextChild::Span(_) => panic!(
                "a `Span` is kept by the `Text` it is written in, which binds its props; \
                 it cannot be kept in a run of children away from one"
            ),
            TextChild::Item(item) => TextNode::item(item.node, item.data),
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
            TextChild::Item(item) => TextNode::item(item.node, item.data),
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

impl Span {
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

pub(crate) type ItemData = bool;

pub(crate) fn text_item(
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
    TextItem {
        node: item.id(),
        data: anchored,
    }
}
