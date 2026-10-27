use std::hash::Hash;
use std::ops::Range;

use beui_core_html::{overlay, scroll};

use crate::reactive::NodeId;
use crate::types::{CaretHandle, Color, Dismiss, ScrollPosition};

pub(crate) fn exists(node: NodeId) -> bool {
    beui_core_html::document::contains(node)
}

pub(crate) fn children(node: NodeId) -> Vec<NodeId> {
    beui_core_html::nodes::children(node)
}

pub(crate) fn focus(node: NodeId) {
    beui_core_html::focus::set_focus(Some(node));
}

pub(crate) fn focus_into(node: NodeId) {
    beui_core_html::nodes::focus_into(node);
}

pub(crate) fn focus_is_within(node: NodeId) -> bool {
    beui_core_html::focus::focus_is_within(node)
}

pub(crate) fn focus_pending_within(_node: NodeId) -> bool {
    false
}

pub(crate) fn focusables_within(node: NodeId) -> Vec<NodeId> {
    beui_core_html::nodes::focusables_within(node)
}

pub(crate) fn raise_overlay(node: NodeId) {
    if let Some(layer) = overlay::find(node) {
        layer.raise();
    }
}

pub(crate) fn dismiss_overlay(node: NodeId, reason: Dismiss) {
    if let Some(layer) = overlay::find(node) {
        layer.dismiss(reason);
    }
}

pub(crate) fn close_overlay(node: NodeId) {
    if let Some(layer) = overlay::find(node) {
        layer.close();
    }
}

pub(crate) fn is_overlay_open(node: NodeId) -> bool {
    overlay::find(node).is_some_and(|layer| layer.is_open())
}

pub(crate) fn reveal(node: NodeId) {
    scroll::reveal(node);
}

pub(crate) fn first_offset_within(node: NodeId) -> Option<NodeId> {
    scroll::first_within(node)
}

pub(crate) fn offset(node: NodeId) -> Option<f32> {
    scroll::find(node).map(|scroller| scroller.offset())
}

pub(crate) fn scroll_position(node: NodeId) -> Option<ScrollPosition> {
    scroll::find(node)?.position()
}

pub(crate) fn drive_offset(node: NodeId, value: f32) {
    if let Some(scroller) = scroll::find(node) {
        scroller.drive(value);
    }
}

pub(crate) fn offset_to_end(node: NodeId) {
    if let Some(scroller) = scroll::find(node) {
        scroller.to_end();
    }
}

pub(crate) fn on_steer(node: NodeId, handler: impl FnMut(()) + 'static) {
    if let Some(scroller) = scroll::find(node) {
        scroller.set_on_steer(crate::reactive::Callback::new(handler));
    }
}

pub(crate) fn row_offset<K: Clone + Hash + Eq + 'static>(_node: NodeId, _key: &K) -> Option<f32> {
    None
}

pub(crate) fn texts_within(_node: NodeId) -> Vec<NodeId> {
    Vec::new()
}

pub(crate) fn text(node: NodeId) -> String {
    beui_core_html::nodes::text(node)
}

pub(crate) fn set_text_selection(_node: NodeId, _selection: Option<(Range<usize>, Color)>) {}

pub(crate) fn set_text_selection_handles(
    _node: NodeId,
    _handles: Vec<(usize, CaretHandle, Color)>,
) {
}
