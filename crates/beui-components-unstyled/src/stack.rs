use beui_macros::{component, view};

use beui_view::types::{Direction, Sizing};
use beui_view::reactive::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[component]
pub fn Stack(spacing: Prop<f32>, narrow: Prop<bool>, children: Children<ListChild>) -> NodeId {
    let stacked = create_memo(move || narrow.get());

    let direction = create_memo(move || {
        if stacked.get() {
            Direction::Vertical
        } else {
            Direction::Horizontal
        }
    });

    let children = children.map(move |child| {
        let ListChild { node, data: size } = child;
        let size = Prop::dynamic(move || {
            let size = size.get();
            if stacked.get() {
                Sizing::default()
            } else {
                size
            }
        });
        ListChild { node, data: size }
    });

    view! {
        <List direction spacing children />
    }
}
