use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Direction;
use crate::types::ScrollPosition;
use beui_macros::component;

/// Shows its one child, laid out at full length along `direction`, from `offset`. It
/// answers no input and never clamps: whatever drives it (a `Scroll`) keeps it in range,
/// and the part past either end is `ScrollPosition::overscroll`.
///
/// With `fit` it measures as long as its child. Without, it measures nothing along its
/// direction (it fills what it is given), except when measured with no limit there (a
/// `Percent` child of a content-sized box): then it measures as long as its child.
///
/// The offset is anchored to the node at the top of the viewport, found by walking down
/// through nodes whose `Element::passes_scroll_anchor` is true, so content above it
/// growing or shrinking leaves what is being read in place. The anchor is picked once a
/// frame from the painted layout. Moving it from outside its layout
/// (`Document::set_offset_value`, a reveal) calls the node's `on_steer`, which a `Scroll`
/// uses to stop a fling.
#[component]
pub fn Offset(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = false)] fit: bool,
    #[prop(default = false)] follow_end: Prop<bool>,
    on_change: Callback<ScrollPosition>,
    children: Child,
) -> NodeId {
    crate::core::offset(offset, direction, fit, follow_end, on_change, children)
}
