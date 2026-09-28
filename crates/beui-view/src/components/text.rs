use crate::reactive::{Prop, create_effect, with_document};
use beui_core::color::Color32;
use beui_core::node::NodeId;

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
    node
}
