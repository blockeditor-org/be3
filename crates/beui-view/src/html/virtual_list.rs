use std::hash::Hash;

use crate::reactive::Callback;
use crate::reactive::ChildSlot;
use crate::reactive::Children;
use crate::reactive::DynamicSegment;
use crate::reactive::Get;
use crate::reactive::KeyedItems;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::ReadSignal;
use crate::reactive::RenderFn;
use crate::reactive::create_binding;
use crate::reactive::create_signal;
use crate::reactive::store;
use crate::types::Direction;
use beui_core_html::css;
use beui_core_html::document::set_style;

use super::bind;

pub fn virtual_list<K>(
    keys: Prop<Vec<K>>,
    item_size: Prop<f32>,
    direction: Prop<Direction>,
    overscan: Prop<f32>,
    text: Callback<K, String>,
    item: RenderFn<ReadSignal<K>>,
) -> NodeId
where
    K: Clone + Hash + Eq + 'static,
{
    let _ = (item_size, overscan, text);
    let list = super::stand_in("VirtualList");
    set_style(list, "display", Some("flex".to_owned()));
    bind(direction, move |direction| {
        set_style(
            list,
            "flex-direction",
            Some(css::flex_direction(direction).to_owned()),
        );
    });
    let rows = DynamicSegment::new(move |slot: ChildSlot<NodeId>| {
        let built = slot.clone();
        let rows = store(KeyedItems::new(move |key: K| {
            built.store(item.call(create_signal(key).read_only()))
        }));
        create_binding(move || {
            let keys = keys.get();
            rows.value().map(keys).commit(|items, _| slot.fill(items));
        });
    });
    Children::from(rows).mount(beui_core_html::tree::node_host(list));
    list
}
