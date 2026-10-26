use beui_view::reactive::NodeId;
use beui_macros::{component, view};
use beui_view::primitives::{self, GridCell};
use beui_view::reactive::*;

use crate::tokens::{Space, resolved};

#[component]
pub fn List(
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Align::Stretch)] align: Prop<Align>,
    #[prop(default = Justify::Start)] justify: Prop<Justify>,
    #[prop(default = false)] wrap: Prop<bool>,
    spacing: Prop<Space>,
    children: Children<ListChild>,
) -> NodeId {
    let spacing = resolved(spacing, Space::resolve);
    view! {
        <primitives::List direction align justify wrap spacing children />
    }
}

#[component]
pub fn Grid(
    columns: Prop<Vec<Track>>,
    #[prop(default = Space::None)] column_spacing: Prop<Space>,
    #[prop(default = Space::None)] row_spacing: Prop<Space>,
    children: Children<GridCell>,
) -> NodeId {
    let column_spacing = resolved(column_spacing, Space::resolve);
    let row_spacing = resolved(row_spacing, Space::resolve);
    view! {
        <primitives::Grid columns column_spacing row_spacing children />
    }
}
