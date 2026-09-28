use beui_macros::{component, view};

use beui_core::base::{Direction, ItemSize};
use beui_core::node::NodeId;
use beui_view::reactive::{Children, List, ListChild, Prop, clone, create_memo};

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

    let children = children.map(move |child| {
        let ListChild { node, size } = child;
        let size = Prop::Dynamic(std::rc::Rc::new(clone!(stacked -> move || {
            if stacked.get() {
                ItemSize::Intrinsic
            } else {
                size.get()
            }
        })));
        ListChild { node, size }
    });

    view! {
        <List direction spacing children />
    }
}
