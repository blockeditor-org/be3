use std::hash::Hash;

use crate::reactive::{Prop, RenderFn, create_effect, with_document};
use beui_core::base::list::Direction;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn VirtualList<K>(
    keys: Prop<Vec<K>>,
    item_size: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(children)] item: RenderFn<K>,
) -> NodeId
where
    K: Clone + Hash + Eq + 'static,
{
    let list = with_document(|document| document.create_virtual_list(move |key| item.call(key)));
    create_effect(move || {
        with_document(|document| document.set_virtual_list_direction::<K>(list, direction.get()))
    });
    create_effect(move || {
        let keys = keys.get();
        with_document(|document| document.set_virtual_list_keys(list, keys));
    });
    create_effect(move || {
        with_document(|document| document.set_virtual_list_item_size::<K>(list, item_size.get()))
    });
    list.id()
}
