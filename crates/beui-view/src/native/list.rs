use crate::reactive::Children;
use crate::reactive::Get;
use crate::reactive::ListChild;
use crate::reactive::Prop;
use crate::reactive::create_binding;
use crate::types::Align;
use crate::types::Direction;
use crate::types::Justify;
use beui_core::current::with_document;
use beui_core::document::Document;
use beui_core::node::NodeId;

pub(crate) fn list(
    direction: Prop<Direction>,
    align: Prop<Align>,
    justify: Prop<Justify>,
    wrap: Prop<bool>,
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

pub(crate) fn spacer() -> NodeId {
    with_document(Document::create_frame).id()
}
