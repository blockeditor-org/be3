use std::cell::Cell;
use std::rc::Rc;

pub use beui_core::base::{Align, Direction, ItemSize, Justify, Sizing, Track};
pub use beui_core::tree::{
    AcceptsSizing, BuildsNode, ListChild, NodeSlot, WithSizing, component_accessibility,
    component_placed, component_rect, component_size, set_component_state, with_sizing,
};
pub use beui_tree::reactive::*;

use beui_core::document::Document;
use beui_core::geometry::{Rect, Vec2};
use beui_core::node::NodeId;

pub use beui_core::callback::NodeRef;
pub use beui_core::timer::{Timer, create_timer, now};

pub use beui_core::current::enter;
pub use beui_core::current::{try_with_document, with_document, with_reactive_scope};

pub type Child = NodeId;

pub type Render<H = (), C = NodeId> = beui_tree::reactive::Render<H, C>;

pub type RenderFn<H, C = NodeId> = beui_tree::reactive::RenderFn<H, C>;

pub fn with_test_id<B: ComponentBuilder>(builder: B, value: impl IntoProp<String>) -> B
where
    B::Output: BuildsNode,
{
    let test_id = value.into_prop();
    builder.with_built(move |built| bind_test_id(built.built_node(), test_id))
}

pub fn with_node_ref<B: ComponentBuilder>(builder: B, value: &NodeRef) -> B
where
    B::Output: BuildsNode,
{
    let node_ref = value.clone();
    builder.with_built(move |built| node_ref.fill(built.built_node()))
}

pub fn build(f: impl FnOnce() -> NodeId) -> Document {
    let mut document = Document::new();
    let root = enter(&mut document, f);
    document.set_root(root);
    document
}

pub fn node_size(node: NodeId) -> ReadSignal<Vec2> {
    with_document(|document| document.watch_size(node))
}

pub fn node_rect(node: NodeId) -> ReadSignal<Rect> {
    with_document(|document| document.watch_placement(node))
}

pub fn node_placed(node: NodeId) -> ReadSignal<bool> {
    with_document(|document| document.watch_placed(node))
}

pub fn layout_text(
    text: &str,
    font: beui_core::font::FontId,
    layout: beui_core::font::TextLayout,
) -> Option<beui_core::font::Galley> {
    try_with_document(|document| document.layout_text(text, font, layout)).flatten()
}

pub fn copy_text(text: impl Into<String>) {
    let text = text.into();
    with_document(|document| document.copy_text(text));
}

pub fn request_paste() {
    with_document(Document::request_paste);
}

pub fn node_scope(document: &Document, owner: Option<ScopeContext>) -> Scope {
    owner
        .or_else(owner_scope)
        .and_then(|owner| owner.child())
        .unwrap_or_else(|| document.reactive_scope().context().run(Scope::new))
}

pub fn pixels_per_point() -> ReadSignal<f32> {
    with_document(|document| document.watch_pixels_per_point())
}

pub fn focus_ring(focused: impl IntoProp<bool>) -> Memo<bool> {
    let focused = focused.into_prop();
    let visible = with_document(|document| document.watch_focus_visible());
    create_memo(move || focused.get() && visible.get())
}

pub fn on_shortcut(shortcut: impl Fn(beui_core::input::KeyPress) -> bool + 'static) {
    let shortcut: Rc<beui_core::document::Shortcut> = Rc::new(shortcut);
    with_document(|document| document.register_shortcut(Rc::downgrade(&shortcut)));
    on_cleanup(move || drop(shortcut));
}

pub fn on_global_key(handler: impl Fn(beui_core::document::GlobalKeyPress) -> bool + 'static) {
    let handler: Rc<beui_core::document::GlobalKey> = Rc::new(handler);
    with_document(|document| document.register_global_key(Rc::downgrade(&handler)));
    on_cleanup(move || drop(handler));
}

pub fn held_modifiers() -> ReadSignal<beui_core::input::Modifiers> {
    with_document(|document| document.watch_modifiers())
}

pub fn on_finger_tap(tap: impl Fn(usize) -> bool + 'static) {
    let tap: Rc<beui_core::document::FingerTap> = Rc::new(tap);
    with_document(|document| document.register_finger_tap(Rc::downgrade(&tap)));
    on_cleanup(move || drop(tap));
}

pub fn last_pointer() -> Option<beui_core::input::PointerSample> {
    try_with_document(|document| document.last_pointer).flatten()
}

pub fn focus_takes_text() -> bool {
    with_document(|document| document.focus_takes_text())
}

pub fn bind(node: NodeId, effect: impl FnMut() + 'static) {
    let scope = with_document(|document| node_scope(document, None));
    scope.context().run(|| create_effect(effect));
    with_document(|document| document.register_node_scope(node, scope));
}

pub fn bind_test_id(node: NodeId, test_id: Prop<String>) {
    let reading = match test_id {
        Prop::Static(value) => {
            with_document(|document| document.set_test_id(node, value));
            return;
        }
        Prop::Dynamic(reading) => reading,
    };
    let published: Cell<Option<String>> = Cell::new(None);
    bind(node, move || {
        let next = reading();
        let previous = published.replace(Some(next.clone()));
        with_document(|document| {
            if let Some(previous) = previous
                && previous != next
            {
                document.clear_test_id(node, &previous);
            }
            document.set_test_id(node, next);
        });
    });
}

pub fn in_new_scope(f: impl FnOnce() -> NodeId) -> NodeId {
    let scope = with_document(|document| node_scope(document, None));
    let node = scope.context().run(f);
    with_document(|document| document.register_node_scope(node, scope));
    node
}

pub use crate::actions::{
    Action, ActionBuilder, ActionScope, Chord, action_disabled, action_glyph, action_label,
    action_pressed, action_scope, action_tooltip, active_actions, active_actions_from,
    menu_actions,
};
pub use crate::components::back::BackHandler;
pub use crate::components::canvas::{Canvas, CanvasItem};
pub use crate::components::drawing::Drawing;
pub use crate::components::embed::Embed;
pub use crate::components::fade::Fade;
pub use crate::components::frame::Frame;
pub use crate::components::grid::{Grid, GridCell};
pub use crate::components::interactive::Interactive;
pub use crate::components::layers::{Layer, Layers};
pub use crate::components::offset::Offset;
pub use crate::components::overlay::Overlay;
pub use crate::components::portal::Portal;
pub use crate::components::shift::Shift;
pub use crate::components::text::{Text, TextItem};
pub use crate::components::virtual_list::VirtualList;
pub use crate::file_picker::{
    FileFilter, FilePick, FilePicker, PickedFile, create_file_picker, pick_file,
};
pub use beui_core::base::canvas::CanvasView;
pub use beui_core::base::drawing::{Draw, draw_gpu};
pub use beui_core::base::embed::{EmbedPlacement, EmbedSlot};
pub use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
pub use beui_core::rich::{
    CaretHandle, RichLayout, SpanKind, SpanStyle, TextCaret, TextMark, TextSpan,
};

#[component]
pub fn List(
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = Align::Stretch)] align: Prop<Align>,
    #[prop(default = Justify::Start)] justify: Prop<Justify>,
    #[prop(default = false)] wrap: Prop<bool>,
    spacing: Prop<f32>,
    children: Children<ListChild>,
) -> NodeId {
    let list = with_document(|document| document.create_list(direction.peek(), 0.0));
    create_effect(move || {
        with_document(|document| document.set_list_direction(list, direction.get()))
    });
    create_effect(move || with_document(|document| document.set_list_align(list, align.get())));
    create_effect(move || with_document(|document| document.set_list_justify(list, justify.get())));
    create_effect(move || with_document(|document| document.set_list_wrap(list, wrap.get())));
    create_effect(move || with_document(|document| document.set_list_spacing(list, spacing.get())));
    children.mount(list);
    list.id()
}

#[component]
pub fn Spacer() -> NodeId {
    with_document(Document::create_frame).id()
}
