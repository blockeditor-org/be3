use crate::reactive::Callback;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::ReadSignal;
use crate::reactive::RenderFn;
use crate::types::Direction;
use beui_macros::component;
use std::hash::Hash;

#[component]
pub fn VirtualList<K>(
    keys: Prop<Vec<K>>,
    item_size: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = 0.0)] overscan: Prop<f32>,
    text: Callback<K, String>,
    #[prop(children)] item: RenderFn<ReadSignal<K>>,
) -> NodeId
where
    K: Clone + Hash + Eq + 'static,
{
    crate::core::virtual_list(keys, item_size, direction, overscan, text, item)
}
