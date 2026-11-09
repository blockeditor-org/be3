use std::hash::Hash;
use std::ops::Range;

use beui_core_html::{overlay, scroll};

use crate::reactive::NodeId;
use crate::types::{CaretHandle, Color, Dismiss, ScrollPosition, TextCaret, TextMark};

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

pub(crate) fn row_offset<K: Clone + Hash + Eq + 'static>(node: NodeId, key: &K) -> Option<f32> {
    super::virtual_list::row_offset(node, key)
}

pub(crate) fn texts_within(node: NodeId) -> Vec<NodeId> {
    beui_core_html::nodes::texts_within(node)
}

pub(crate) fn text(node: NodeId) -> String {
    beui_core_html::nodes::text(node)
}

pub(crate) fn set_text_selection(node: NodeId, selection: Option<(Range<usize>, Color)>) {
    if let Some(model) = beui_core_html::text::model(node) {
        model.set_selection(selection.map(|(range, color)| TextMark::new(range, color)));
    }
}

pub(crate) fn set_text_selection_handles(node: NodeId, handles: Vec<(usize, CaretHandle, Color)>) {
    if let Some(model) = beui_core_html::text::model(node) {
        model.set_handles(
            handles
                .into_iter()
                .map(|(at, handle, color)| TextCaret {
                    handle: Some(handle),
                    ..TextCaret::new(at, color, 0.0)
                })
                .collect(),
        );
    }
}
