use crate::reactive::{
    BuildsNode, Child, ChildValue, Children, NodeSlot, Prop, Scope, SlotChild, create_effect,
    with_document,
};
use beui_core::base::text::TextNode;
use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_core::rich::{TextCaret, TextMark, TextSpan};

use beui_core::base::text::DEFAULT_FONT_SIZE;
use beui_core::font::TextAlign;
use beui_macros::component;

#[component]
pub fn Text(
    string: Prop<String>,
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
    spans: Option<Prop<Vec<TextSpan>>>,
    #[prop(default = (0.0, 0.0))] line_padding: Prop<(f32, f32)>,
    #[prop(default = Vec::new())] marks: Prop<Vec<TextMark>>,
    #[prop(default = Vec::new())] carets: Prop<Vec<TextCaret>>,
    children: Children<TextItem>,
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
    if let Some(spans) = spans {
        create_effect(move || with_document(|document| document.set_text_spans(node, spans.get())));
        create_effect(move || {
            with_document(|document| document.set_text_line_padding(node, line_padding.get()))
        });
        create_effect(move || with_document(|document| document.set_text_marks(node, marks.get())));
        create_effect(move || {
            with_document(|document| document.set_text_carets(node, carets.get()))
        });
    }
    children.mount(node);
    node.id()
}

pub struct TextItem {
    node: NodeId,
}

impl BuildsNode for TextItem {
    fn built_node(&self) -> NodeId {
        self.node
    }
}

impl ChildValue for TextItem {
    fn anchor(&self) -> Option<NodeId> {
        Some(self.node)
    }

    fn adopt_scope(&mut self, scope: Scope) {
        self.node.adopt_scope(scope);
    }
}

crate::child_type!(TextItem);

impl SlotChild for TextItem {
    type Stored = NodeId;

    fn store(self, _parent: Option<NodeId>) -> NodeId {
        self.node
    }

    fn stored_node(stored: &NodeId) -> Option<NodeId> {
        Some(*stored)
    }
}

impl NodeSlot for TextItem {
    type Host = TextNode;
}

#[component]
pub fn TextItem(
    #[prop(default = None)] at: Prop<Option<usize>>,
    children: Option<Child>,
) -> TextItem {
    let item = with_document(|document| {
        let item = document.create_text_item();
        if let Some(child) = children {
            document.set_text_item_child(item, child);
        }
        item
    });
    create_effect(move || with_document(|document| document.set_text_item_at(item, at.get())));
    TextItem { node: item.id() }
}
