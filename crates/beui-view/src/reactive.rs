pub use beui_tree::reactive::*;

pub use crate::component_ref::ComponentRef;
#[cfg(not(beui_html))]
pub use crate::core::reactive::*;
pub use crate::core::reactive::{
    Draw, ImeCursor, KeyAt, KeyEvent, NodeId, NodeRef, OverlayAnchor, Timer, bind, bind_test_id,
    copy_text, create_timer, focus_ancestry, focus_outline, focus_ring, focused_node, holds_focus,
    now, on_global_key, on_key, on_shortcut, pin_wall_clock, pixels_per_point, set_focus,
    tree_local, use_font_revision, use_preferences, use_viewport_size, wall_clock,
};
pub use crate::core::tree::{
    BareChild, BuildsNode, FromPlaced, ListChild, ParentKind, Placed, WithSizing,
    component_accessibility, component_placed, with_sizing, with_sizing as sizing,
};
pub use crate::measure::{LineBox, TextGeometry, measure, measure_text};

pub type Child = NodeId;

pub type Render<H = (), C = NodeId> = beui_tree::reactive::Render<H, C>;

pub type RenderFn<H, C = NodeId> = beui_tree::reactive::RenderFn<H, C>;

pub fn test_id<T: BuildsNode>(built: T, value: impl IntoProp<String>) -> T {
    bind_test_id(built.built_node(), value.into_prop());
    built
}

pub fn node_ref<T: BuildsNode>(built: T, reference: &NodeRef) -> T {
    reference.fill(built.built_node());
    built
}
