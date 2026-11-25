use crate::frame::{Border, ColorRole, Frame, Radius};

use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Extent;

use crate::tokens::{Length, Padding, Size, Space};

pub(crate) const PADDING: Space = Space::Xl;

/// A card's corners.
pub const RADIUS: Radius = Radius::Lg;
/// A card's border.
pub const BORDER: Border = Border::DEFAULT;
/// What keeps a child filling a card (a child frame a dialog holds) inside the card's border.
pub const INSIDE_BORDER: Padding = Padding::all(Space::Custom(BORDER.width.px()));
/// The corners inside a card's border, which such a child is rounded by.
pub const INNER_RADIUS: Radius = Radius::Custom(RADIUS.px() - BORDER.width.px());

/// The raised surface every card, popover and dialog sits on: the surface fill, the default
/// border and a large radius. `padding` is a page's card's by default (a popover's is
/// `PANEL_PADDING`, a dialog's body pads itself), `max_width` caps it and otherwise it is as
/// wide as its content, and an overlay with a `beak` points it from the frame bound to
/// `pointer` (`Dropdown::pointer`, `DialogCardHandle::pointer`).
#[component]
pub fn Card(
    #[prop(default = Padding::all(PADDING))] padding: Prop<Padding>,
    #[prop(default = None)] max_width: Prop<Option<Size>>,
    #[prop(default = NodeRef::new())] pointer: NodeRef,
    children: Child,
) -> NodeId {
    view! {
        <Frame
            @node_ref=&pointer
            fill=ColorRole::Surface
            border={BORDER}
            radius=RADIUS
            padding
            width={max_width.map(|max| max.map_or(Length::INTRINSIC, at_most))}
        >
            {children}
        </Frame>
    }
}

fn at_most(size: Size) -> Length {
    Length::Extent(Extent::at_most(size.px()))
}
