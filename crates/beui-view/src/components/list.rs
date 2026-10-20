use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_macros::component;

use crate::reactive::{
    Align, Children, Direction, Get, Justify, ListChild, Prop, create_binding, with_document,
};

#[component]
pub fn List(
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Align::Stretch)] align: Prop<Align>,
    #[prop(default = Justify::Start)] justify: Prop<Justify>,
    #[prop(default = false)] wrap: Prop<bool>,
    spacing: Prop<f32>,
    children: Children<ListChild>,
) -> NodeId {
    let list = with_document(|document| document.create_list(direction.get_untracked(), 0.0));
    create_binding(move || {
        with_document(|document| document.set_list_direction(list, direction.get()))
    });
    create_binding(move || with_document(|document| document.set_list_align(list, align.get())));
    create_binding(move || {
        with_document(|document| document.set_list_justify(list, justify.get()))
    });
    create_binding(move || with_document(|document| document.set_list_wrap(list, wrap.get())));
    create_binding(move || {
        with_document(|document| document.set_list_spacing(list, spacing.get()))
    });
    children.mount(list);
    list.id()
}

#[component]
pub fn Spacer() -> NodeId {
    with_document(Document::create_frame).id()
}
