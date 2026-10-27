use std::hash::Hash;
use std::ops::Range;

use crate::core::nodes as core;
use crate::reactive::NodeId;
use crate::types::{CaretHandle, Color32, Dismiss, Rect, ScrollPosition};

pub trait Inspect {
    fn api<A: Clone + 'static>(&self, node: NodeId) -> A;

    fn node_rect(&self, node: NodeId) -> Option<Rect>;

    fn is_overlay_open(&self, node: NodeId) -> bool;
}

pub trait TreeNode {
    fn exists(&self) -> bool;
    fn children(&self) -> Vec<NodeId>;
    fn focus(&self);
    fn focus_into(&self);
    fn focus_is_within(&self) -> bool;
    fn focus_pending_within(&self) -> bool;
    fn focusables_within(&self) -> Vec<NodeId>;
    fn raise_overlay(&self);
    fn dismiss_overlay(&self, reason: Dismiss);
    fn close_overlay(&self);
    fn is_overlay_open(&self) -> bool;
    fn reveal(&self);
    fn first_offset_within(&self) -> Option<NodeId>;
    fn offset(&self) -> Option<f32>;
    fn scroll_position(&self) -> Option<ScrollPosition>;
    fn drive_offset(&self, value: f32);
    fn offset_to_end(&self);
    fn on_steer(&self, handler: impl FnMut(()) + 'static);
    fn row_offset<K: Clone + Hash + Eq + 'static>(&self, key: &K) -> Option<f32>;
    fn texts_within(&self) -> Vec<NodeId>;
    fn text(&self) -> String;
    fn set_text_selection(&self, selection: Option<(Range<usize>, Color32)>);
    fn set_text_selection_handles(&self, handles: Vec<(usize, CaretHandle, Color32)>);
}

impl TreeNode for NodeId {
    fn exists(&self) -> bool {
        core::exists(*self)
    }

    fn children(&self) -> Vec<NodeId> {
        core::children(*self)
    }

    fn focus(&self) {
        core::focus(*self);
    }

    fn focus_into(&self) {
        core::focus_into(*self);
    }

    fn focus_is_within(&self) -> bool {
        core::focus_is_within(*self)
    }

    fn focus_pending_within(&self) -> bool {
        core::focus_pending_within(*self)
    }

    fn focusables_within(&self) -> Vec<NodeId> {
        core::focusables_within(*self)
    }

    fn raise_overlay(&self) {
        core::raise_overlay(*self);
    }

    fn dismiss_overlay(&self, reason: Dismiss) {
        core::dismiss_overlay(*self, reason);
    }

    fn close_overlay(&self) {
        core::close_overlay(*self);
    }

    fn is_overlay_open(&self) -> bool {
        core::is_overlay_open(*self)
    }

    fn reveal(&self) {
        core::reveal(*self);
    }

    fn first_offset_within(&self) -> Option<NodeId> {
        core::first_offset_within(*self)
    }

    fn offset(&self) -> Option<f32> {
        core::offset(*self)
    }

    fn scroll_position(&self) -> Option<ScrollPosition> {
        core::scroll_position(*self)
    }

    fn drive_offset(&self, value: f32) {
        core::drive_offset(*self, value);
    }

    fn offset_to_end(&self) {
        core::offset_to_end(*self);
    }

    fn on_steer(&self, handler: impl FnMut(()) + 'static) {
        core::on_steer(*self, handler);
    }

    fn row_offset<K: Clone + Hash + Eq + 'static>(&self, key: &K) -> Option<f32> {
        core::row_offset(*self, key)
    }

    fn texts_within(&self) -> Vec<NodeId> {
        core::texts_within(*self)
    }

    fn text(&self) -> String {
        core::text(*self)
    }

    fn set_text_selection(&self, selection: Option<(Range<usize>, Color32)>) {
        core::set_text_selection(*self, selection);
    }

    fn set_text_selection_handles(&self, handles: Vec<(usize, CaretHandle, Color32)>) {
        core::set_text_selection_handles(*self, handles);
    }
}
