pub use beui_tree::reactive::*;

use std::time::{Duration, Instant};

use crate::core::reactive as core;
use crate::types::{
    BackGesture, Color, Direction, KeyPhase, Modifiers, Outline, Pos2, PreferenceLayer,
    PreferenceLayers, PreferenceSource, Preferences, Vec2, WindowId,
};

pub use crate::actions::{
    Action, ActionBuilder, ActionScope, Chord, MenuNode, Pin, Submenu, action_checked,
    action_disabled, action_glyph, action_label, action_scope, action_tooltip, active_actions,
    active_actions_from, add_to_menu, menu_action, menu_scope, tidy_menu,
};
pub use crate::animation::{
    Animation, AnimationFrame, Easing, Fling, Motion, Spring, Velocity, create_animation, motion,
    tween,
};
pub use crate::colors::{color_scheme, define_colors, resolve_color};
pub use crate::component_ref::ComponentRef;
pub use crate::components::list::ListItem;
pub use crate::file_picker::{FileFilter, FilePick, FilePicker, create_file_picker, pick_file};
pub use crate::file_saver::{FileSave, SavedFile, save_file};
pub use crate::measure::{
    LineBox, Measured, TextGeometry, TextGeometrySignal, lay_out_text, measure, measure_text,
};
pub use crate::nodes::{Inspect, TreeNode};
pub use crate::painter::{Draw, Painter, TextBlock};
pub use crate::types::clipboard::formats;
pub use crate::types::keys::{KeyAt, KeyEvent, KeyInput, KeyReveal};
pub use crate::types::layer::OverlayAnchor;
pub use crate::types::node::{BuildsNode, ImeCursor, NamesNode, NodeId, NodeRef, create_host};
pub use crate::types::tree::{BareChild, FromPlaced, ParentKind, Placed};
pub use crate::types::{Clip, ClipItem};

pub type Child = NodeId;

pub type Render<H = (), C = NodeId> = beui_tree::reactive::Render<H, C>;

pub type RenderFn<H, C = NodeId> = beui_tree::reactive::RenderFn<H, C>;

/// What `@test_id` names: a node, control flow that builds at most one (`NamesNode`),
/// and a value child (`ChoiceOption`, `MenuItem`) whose parent binds the id on the row it
/// builds for it.
#[diagnostic::on_unimplemented(
    message = "a `{Self}` is no node, so `@test_id` has nothing to name",
    label = "a run of children has no node of its own: put `@test_id` on a tag inside each row, or on a tag around the run"
)]
pub trait TakesTestId: Sized {
    type Named;

    fn take_test_id(self, test_id: Prop<String>) -> Self::Named;
}

impl<T: NamesNode> TakesTestId for T {
    type Named = T::Named;

    fn take_test_id(self, test_id: Prop<String>) -> T::Named {
        let (named, node) = self.name_node();
        bind_test_id(node, test_id);
        named
    }
}

pub fn test_id<T: TakesTestId, M1>(built: T, value: impl IntoPropWith<String, M1>) -> T::Named {
    built.take_test_id(value.into_prop_with())
}

pub fn node_ref<T: NamesNode>(built: T, reference: &NodeRef) -> T::Named {
    let (named, node) = built.name_node();
    reference.fill(node);
    named
}

/// Owned by the reactive scope that created it; calling it after that scope is gone does
/// nothing.
#[derive(Clone, Copy)]
pub struct Timer(core::Timer);

impl Timer {
    /// Keeps an earlier deadline if one is set; `restart` replaces it.
    pub fn start(&self, delay: Duration) {
        self.0.start(delay);
    }

    pub fn restart(&self, delay: Duration) {
        self.0.restart(delay);
    }

    pub fn stop(&self) {
        self.0.stop();
    }

    pub fn running(&self) -> bool {
        self.0.running()
    }
}

/// beui has no per-frame hook: anything that depends on time asks for it here. Due timers
/// run at the top of a frame and the document asks for a repaint at the next deadline, so
/// nothing is drawn while none is due. `work` returns `Some(delay)` to run again and
/// `None` once settled. A program settles for `drive` only once no frame is asked for
/// immediately, so endless motion (an indeterminate progress bar) should ask for its next
/// step a frame's time later (`Some(Duration::from_millis(16))`), not `Duration::ZERO`.
pub fn create_timer(work: impl Fn() -> Option<Duration> + 'static) -> Timer {
    Timer(core::create_timer(work))
}

/// The frame clock: tests stop and step it, so read time here, never `Instant::now()`.
pub fn now() -> Instant {
    core::now()
}

pub fn wall_clock() -> Duration {
    core::wall_clock()
}

pub fn pin_wall_clock(at: Option<(Instant, Duration)>) {
    core::pin_wall_clock(at);
}

/// Whether the wall clock is pinned, as tests pin it.
pub fn wall_clock_is_pinned() -> bool {
    core::wall_clock_is_pinned()
}

pub fn after_input(after: impl FnOnce() + 'static) {
    core::after_input(after);
}

pub fn component_node() -> NodeRef {
    crate::core::tree::component_node()
}

pub fn component_placed() -> Reader<bool> {
    crate::core::tree::component_placed()
}

pub fn accessibility_reference(node: NodeId) -> accesskit::NodeId {
    crate::core::tree::accessibility_reference(node)
}

pub fn component_accessibility<M1>(node: impl IntoPropWith<accesskit::Node, M1>) {
    crate::core::tree::component_accessibility(node.into_prop_with());
}

pub fn bind(node: NodeId, effect: impl FnMut() + 'static) {
    core::bind(node, effect);
}

pub fn bind_test_id(node: NodeId, test_id: Prop<String>) {
    core::bind_test_id(node, test_id);
}

pub fn tree_local<T: Clone + 'static>(make: impl FnOnce() -> T) -> T {
    core::tree_local(make)
}

/// Puts `clip` on the clipboard: `Clip::text` for plain text, or several
/// representations of the same content, richest first.
pub fn copy(clip: crate::types::Clip) {
    core::copy(clip);
}

pub fn request_paste() {
    core::request_paste();
}

pub fn type_events(events: Vec<crate::types::Input>) {
    core::type_events(events);
}

pub fn request_typed_paste() {
    core::request_typed_paste();
}

pub fn toggle_keyboard() {
    core::toggle_keyboard();
}

pub fn close_keyboard() {
    core::close_keyboard();
}

pub fn window_of(node: NodeId) -> WindowId {
    core::window_of(node)
}

pub fn opener_of(node: NodeId) -> NodeId {
    core::opener_of(node)
}

pub fn stacked_at(nodes: &[NodeId], window: WindowId, pos: Pos2) -> Vec<NodeId> {
    core::stacked_at(nodes, window, pos)
}

pub fn focused_node() -> Option<NodeId> {
    core::focused_node()
}

pub fn set_focus(node: Option<NodeId>) {
    core::set_focus(node);
}

pub fn focus_ancestry() -> Vec<NodeId> {
    core::focus_ancestry()
}

pub fn focus_takes_text() -> bool {
    core::focus_takes_text()
}

pub fn focus_ring<M1>(focused: impl IntoPropWith<bool, M1>) -> Memo<bool> {
    core::focus_ring(focused.into_prop_with())
}

pub fn focus_visible() -> bool {
    core::focus_visible()
}

pub fn holds_focus(node: NodeRef) -> Memo<bool> {
    core::holds_focus(node)
}

/// Whether any node of the document has the focus.
pub fn focus_held() -> Memo<bool> {
    core::focus_held()
}

pub fn focus_outline<M1, M2>(
    focused: impl IntoPropWith<bool, M1>,
    color: impl IntoPropWith<Color, M2>,
    width: f32,
    offset: f32,
) -> Memo<Option<Outline>> {
    let ring = focus_ring(focused);
    let color = color.into_prop_with();
    create_memo(move || {
        ring.get()
            .then(|| Outline::new(color.get(), width).offset(offset))
    })
}

pub fn keep_clear<M1>(node: NodeId, rects: impl IntoPropWith<Vec<crate::types::Rect>, M1>) {
    core::keep_clear(node, rects.into_prop_with());
}

/// Keeps `node` above embeds painted before it: see `AboveEmbeds`.
pub fn above_embeds<M1>(
    node: NodeId,
    above: impl IntoPropWith<Option<crate::types::AboveEmbeds>, M1>,
) {
    core::above_embeds(node, above.into_prop_with());
}

pub fn window_point(pos: Pos2, from: WindowId, to: WindowId) -> Option<Pos2> {
    match from == to {
        true => Some(pos),
        false => Some(to_host(to)?.inverse_point(to_host(from)?.point(pos))),
    }
}

pub fn to_host(window: WindowId) -> Option<crate::types::HostTransform> {
    core::to_host(window)
}

pub fn arrow_focus<M1>(row: NodeRef, direction: impl IntoPropWith<Direction, M1>) {
    core::arrow_focus(row, direction.into_prop_with());
}

pub fn held_modifiers() -> ReadSignal<Modifiers> {
    core::held_modifiers()
}

pub fn on_key(
    at: KeyAt,
    phase: KeyPhase,
    reveal: KeyReveal,
    handler: impl Fn(KeyEvent) -> bool + 'static,
) {
    core::on_key(at, phase, reveal, handler);
}

pub fn on_back<M1>(enabled: impl IntoPropWith<bool, M1>, handler: impl Fn(BackGesture) + 'static) {
    core::on_back(enabled.into_prop_with(), handler);
}

pub fn on_finger_tap(tap: impl Fn(usize) -> bool + 'static) {
    core::on_finger_tap(tap);
}

pub fn touched_by_finger() -> bool {
    core::touched_by_finger()
}

/// Whether the document has the keyboard; in an app, true while any of the app's views has it.
/// An editor asks `Editor::has_keyboard` whether one of its own views has it.
pub fn use_document_keyboard() -> ReadSignal<bool> {
    core::use_document_keyboard()
}

pub fn use_fullscreen() -> ReadSignal<bool> {
    core::use_fullscreen()
}

pub fn pixels_per_point() -> ReadSignal<f32> {
    core::pixels_per_point()
}

pub fn use_viewport_size() -> Memo<Vec2> {
    let viewport = core::use_viewport_size();
    let window = crate::window::use_window();
    create_memo(move || match window.filter(|window| window.is_window()) {
        Some(window) => window.size.get(),
        None => viewport.get(),
    })
}

/// Notified when the document's fonts change, so text laid out outside the tree
/// (`lay_out_text`) is measured again.
pub fn use_font_changes() -> Trigger {
    core::use_font_changes()
}

pub fn use_preferences() -> Memo<Preferences> {
    core::use_preferences()
}

pub fn use_preference_layers() -> Memo<PreferenceLayers> {
    core::use_preference_layers()
}

pub fn use_preference_layer(source: PreferenceSource) -> Binding<PreferenceLayer> {
    core::use_preference_layer(source)
}
