use std::hash::Hash;
use std::ops::Range;

use beui_core_html::scroll;

use crate::reactive::{Get, NodeId};
use crate::types::{CaretHandle, Color32, Dismiss, Rect, ScrollPosition};

#[derive(Clone, Copy, Debug, Default)]
pub struct Page;

impl crate::nodes::Inspect for Page {
    fn api<A: Clone + 'static>(&self, node: NodeId) -> A {
        beui_core_html::document::state::<A>(node)
            .unwrap_or_else(|| panic!("component has no {} state", std::any::type_name::<A>()))
    }

    fn node_rect(&self, node: NodeId) -> Option<Rect> {
        crate::measure::measure(node).get_untracked()
    }

    fn is_overlay_open(&self, node: NodeId) -> bool {
        is_overlay_open(node)
    }
}

pub fn exists(node: NodeId) -> bool {
    beui_core_html::document::contains(node)
}

pub fn children(node: NodeId) -> Vec<NodeId> {
    beui_core_html::nodes::children(node)
}

pub fn focus(node: NodeId) {
    beui_core_html::focus::set_focus(Some(node));
}

pub fn focus_into(node: NodeId) {
    beui_core_html::nodes::focus_into(node);
}

pub fn focus_is_within(node: NodeId) -> bool {
    beui_core_html::focus::focus_is_within(node)
}

pub fn focus_pending_within(_node: NodeId) -> bool {
    false
}

pub fn focusables_within(node: NodeId) -> Vec<NodeId> {
    beui_core_html::nodes::focusables_within(node)
}

pub fn raise_overlay(_node: NodeId) {}

pub fn dismiss_overlay(_node: NodeId, _reason: Dismiss) {}

pub fn close_overlay(_node: NodeId) {}

pub fn is_overlay_open(node: NodeId) -> bool {
    beui_core_html::dom::connected(node)
}

pub fn reveal(node: NodeId) {
    scroll::reveal(node);
}

pub fn first_offset_within(node: NodeId) -> Option<NodeId> {
    scroll::first_within(node)
}

pub fn offset(node: NodeId) -> Option<f32> {
    scroll::find(node).map(|scroller| scroller.offset())
}

pub fn scroll_position(node: NodeId) -> Option<ScrollPosition> {
    scroll::find(node)?.position()
}

pub fn drive_offset(node: NodeId, value: f32) {
    if let Some(scroller) = scroll::find(node) {
        scroller.drive(value);
    }
}

pub fn offset_to_end(node: NodeId) {
    if let Some(scroller) = scroll::find(node) {
        scroller.to_end();
    }
}

pub fn on_steer(node: NodeId, handler: impl FnMut(()) + 'static) {
    if let Some(scroller) = scroll::find(node) {
        scroller.set_on_steer(crate::reactive::Callback::new(handler));
    }
}

pub fn row_offset<K: Clone + Hash + Eq + 'static>(_node: NodeId, _key: &K) -> Option<f32> {
    None
}

pub fn texts_within(_node: NodeId) -> Vec<NodeId> {
    Vec::new()
}

pub fn text(node: NodeId) -> String {
    beui_core_html::nodes::text(node)
}

pub fn set_text_selection(_node: NodeId, _selection: Option<(Range<usize>, Color32)>) {}

pub fn set_text_selection_handles(_node: NodeId, _handles: Vec<(usize, CaretHandle, Color32)>) {}
