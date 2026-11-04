use beui_macros::{component, view};
use beui_view::primitives::{self, GridCell};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

use crate::theme::Density;
use crate::tokens::{Space, resolved};
use beui_view::types::Extent;
use beui_view::types::{Align, Direction, Justify, Track};

#[component]
pub fn List(
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Align::Stretch)] align: Prop<Align>,
    #[prop(default = Justify::Start)] justify: Prop<Justify>,
    #[prop(default = false)] wrap: Prop<bool>,
    spacing: Prop<Space>,
    children: Children<ListItem>,
) -> NodeId {
    let spacing = resolved(spacing, Space::resolve);
    view! {
        <primitives::List direction align justify wrap spacing children />
    }
}

fn spaced(space: Space, density: Density) -> Extent {
    match space {
        Space::None => Extent::INTRINSIC,
        space => {
            let length = space.resolve(density);
            Extent::px(length).min(length)
        }
    }
}

/// An empty child of a `List` or `Stack`. With neither `width` nor `height` it grows,
/// taking the length the other children leave; with either it is that size and does not
/// grow. `grow` sets the weight either way.
#[component]
pub fn Spacer(
    width: Option<Prop<Space>>,
    height: Option<Prop<Space>>,
    grow: Option<Prop<f32>>,
) -> ListItem {
    let sized = width.is_some() || height.is_some();
    let grow = grow.unwrap_or(Prop::Static(match sized {
        true => 0.0,
        false => 1.0,
    }));
    let width = resolved(width.unwrap_or(Prop::Static(Space::None)), spaced);
    let height = resolved(height.unwrap_or(Prop::Static(Space::None)), spaced);
    view! {
        <primitives::ListItem grow>
            <primitives::Frame width height />
        </primitives::ListItem>
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
