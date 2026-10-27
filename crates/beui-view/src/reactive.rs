pub use beui_tree::reactive::*;

pub use crate::actions::{
    Action, ActionBuilder, ActionScope, Chord, MenuNode, Pin, Submenu, action_checked,
    action_disabled, action_glyph, action_label, action_scope, action_tooltip, active_actions,
    active_actions_from, add_to_menu, document_menu, menu_action, tidy_menu,
};
pub use crate::animation::{
    Animation, AnimationFrame, Easing, Fling, Motion, Spring, create_animation, motion, tween,
};
pub use crate::colors::{color_scheme, define_colors, resolve_color};
pub use crate::component_ref::ComponentRef;
#[cfg(not(beui_html))]
pub use crate::core::reactive::*;
pub use crate::core::reactive::{
    Draw, ImeCursor, KeyAt, KeyEvent, KeyInput, NodeId, NodeRef, OverlayAnchor, Painter, Timer,
    after_input, arrow_focus, bind, bind_test_id, copy_text, create_timer, focus_ancestry,
    focus_outline, focus_ring, focused_node, held_modifiers, holds_focus, now, on_back,
    on_global_key, on_key, on_scroll_key, on_shortcut, pin_wall_clock, pixels_per_point,
    request_paste, set_focus, stacked_at, touched_by_finger, tree_local, use_font_revision,
    use_keyboard, use_preference_layer, use_preference_layers, use_preferences, use_viewport_size,
    wall_clock, window_of,
};
pub use crate::core::tree::{
    BareChild, BuildsNode, FromPlaced, ListChild, ParentKind, Placed, WithSizing,
    component_accessibility, component_node, component_placed, with_sizing, with_sizing as sizing,
};
pub use crate::file_picker::{FileFilter, FilePick, FilePicker, create_file_picker, pick_file};
pub use crate::file_saver::{FileSave, SavedFile, save_file};
pub use crate::measure::{LineBox, Measured, TextGeometry, measure, measure_line, measure_text};
pub use crate::nodes::{Inspect, TreeNode};

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
