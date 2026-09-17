use beui_macros::{component, view};

use crate::base::{Direction, ItemSize};
use crate::node::NodeId;
use crate::reactive::{Children, List, ListChild, Prop, clone, create_memo};

#[component]
pub fn Stack(spacing: Prop<f32>, narrow: Prop<bool>, children: Children<ListChild>) -> NodeId {
    let stacked = create_memo(move || narrow.get());

    let direction = create_memo(clone!(stacked -> move || {
        if stacked.get() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        }
    }));

    let children: Vec<ListChild> = children
        .into_items()
        .into_iter()
        .map(|child| {
            let ListChild { node, size } = child;
            let size = Prop::Dynamic(Box::new(clone!(stacked -> move || {
                if stacked.get() {
                    ItemSize::Intrinsic
                } else {
                    size.get()
                }
            })));
            ListChild { node, size }
        })
        .collect();

    view! {
        <List direction spacing children />
    }
}
