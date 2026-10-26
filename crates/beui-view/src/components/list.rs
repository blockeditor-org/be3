use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_macros::component;

use crate::reactive::{Children, Get, ListChild, Prop, create_binding, with_document};

use crate::types::{Align, Direction, Justify};

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
        let direction = direction.get();
        with_document(|document| document.set_list_direction(list, direction))
    });
    create_binding(move || {
        let align = align.get();
        with_document(|document| document.set_list_align(list, align))
    });
    create_binding(move || {
        let justify = justify.get();
        with_document(|document| document.set_list_justify(list, justify))
    });
    create_binding(move || {
        let wrap = wrap.get();
        with_document(|document| document.set_list_wrap(list, wrap))
    });
    create_binding(move || {
        let spacing = spacing.get();
        with_document(|document| document.set_list_spacing(list, spacing))
    });
    children.mount(list);
    list.id()
}

#[component]
pub fn Spacer() -> NodeId {
    with_document(Document::create_frame).id()
}
