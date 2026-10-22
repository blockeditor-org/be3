use std::hash::Hash;
use std::ops::Range;
use std::rc::Rc;

use beui_core::animation::Motion;
use beui_core::base::offset::{OffsetNode, ScrollPosition};
use beui_core::base::overlay::OverlayNode;
use beui_core::base::text::{TextGeometry, TextNode};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::drag_board::Board;
use beui_core::geometry::{Pos2, Rect};
use beui_core::layer::{Dismiss, LayerPolicy};
use beui_core::node::{NodeId, NodeOf, Rects};
use beui_core::preferences::{PreferenceLayer, PreferenceLayers, PreferenceSource, ThemeKind};
use beui_core::rich::CaretHandle;
use beui_tree::reactive::ReadSignal;

use beui_core::current::{try_with_document, with_document};

pub trait Inspect {
    fn component_state<T: 'static>(&self, node: NodeId) -> &T;

    fn node_rect(&self, node: NodeId) -> Option<Rect>;

    fn is_overlay_open(&self, node: NodeId) -> bool;
}

impl Inspect for Document {
    fn component_state<T: 'static>(&self, node: NodeId) -> &T {
        Document::component_state(self, node)
    }

    fn node_rect(&self, node: NodeId) -> Option<Rect> {
        Document::node_rect(self, node)
    }

    fn is_overlay_open(&self, node: NodeId) -> bool {
        Document::is_overlay_open(self, node)
    }
}

#[derive(Clone)]
pub struct Placements(Rc<Rects>);

impl Placements {
    pub fn rect(&self, node: NodeId) -> Option<Rect> {
        self.0.get(&node)
    }
}

pub fn placements() -> Placements {
    Placements(with_document(|document| Rc::clone(&document.rects)))
}

pub fn component_state<T: Clone + 'static>(node: NodeId) -> T {
    with_document(|document| document.component_state::<T>(node).clone())
}

pub fn viewport_rect() -> Rect {
    with_document(|document| document.viewport_rect())
}

pub fn node_exists(node: NodeId) -> bool {
    with_document(|document| document.contains(node))
}

pub fn node_children(node: NodeId) -> Vec<NodeId> {
    with_document(|document| match document.contains(node) {
        true => document.children(node),
        false => Vec::new(),
    })
}

pub fn tree_local<T: Clone + 'static>(make: impl FnOnce() -> T) -> T {
    with_document(|document| document.extension_or_insert_with(make))
}

pub fn use_motion() -> ReadSignal<Motion> {
    with_document(|document| document.watch_motion())
}

pub fn use_theme_kind() -> ReadSignal<ThemeKind> {
    with_document(|document| document.watch_theme())
}

pub fn use_preference_layers() -> ReadSignal<PreferenceLayers> {
    with_document(|document| document.watch_preference_layers())
}

pub fn update_preference_layer(
    source: PreferenceSource,
    change: impl FnOnce(PreferenceLayer) -> PreferenceLayer,
) {
    with_document(|document| {
        if let Some(context) = document.context() {
            let layer = context.preference_layer(source);
            context.set_preference_layer(source, change(layer));
        }
    });
}

pub fn use_attached() -> ReadSignal<u64> {
    with_document(|document| document.watch_context())
}

pub fn set_pointer_locked(locked: bool) {
    try_with_document(|document| {
        if let Some(context) = document.context() {
            context.set_pointer_locked(locked);
        }
    });
}

pub fn drag_board() -> Rc<Board> {
    with_document(|document| document.drag_board())
}

pub fn focus_is_within(node: NodeId) -> bool {
    with_document(|document| document.focus_is_within(node))
}

pub fn focus_pending_within(node: NodeId) -> bool {
    with_document(|document| document.focus_pending_within(node))
}

pub fn focusables_within(node: NodeId) -> Vec<NodeId> {
    with_document(|document| document.focusables_within(node))
}

pub fn focused_node() -> Option<NodeId> {
    with_document(|document| document.focused_node())
}

pub fn focus_node(node: NodeId) {
    with_document(|document| document.focus_focusable(node));
}

pub fn set_focus(node: Option<NodeId>) {
    with_document(|document| document.update_focus(node));
}

pub fn focus_into(node: NodeId) {
    with_document(|document| document.focus_into(node));
}

pub fn focus_ancestry() -> Vec<NodeId> {
    with_document(|document| document.focus_ancestry())
}

pub fn raise_overlay(overlay: NodeId) {
    with_document(|document| document.raise_overlay(overlay));
}

pub fn dismiss_overlay(overlay: NodeId, reason: Dismiss) {
    with_document(|document| document.dismiss_overlay(overlay, reason));
}

pub fn close_overlay(overlay: NodeId) {
    with_document(|document| {
        if let Some(overlay) = document.arena.kind_of::<OverlayNode>(overlay) {
            document.close_overlay(overlay);
        }
    });
}

pub fn is_overlay_open(overlay: NodeId) -> bool {
    with_document(|document| document.is_overlay_open(overlay))
}

pub fn stacked_at(nodes: &[NodeId], pos: Pos2) -> Vec<NodeId> {
    with_document(|document| {
        let layers = document.layers();
        let mut placed: Vec<(usize, NodeId)> = nodes
            .iter()
            .filter_map(|node| Some((document.layer_of(&layers, *node)?, *node)))
            .collect();
        placed.sort_by_key(|(layer, _)| std::cmp::Reverse(*layer));
        let foreign = (1..layers.len()).rev().find(|index| {
            layers[*index].policy != LayerPolicy::Inert
                && !placed.iter().any(|(layer, _)| layer == index)
                && document
                    .layer_covers(&layers[*index])
                    .iter()
                    .any(|cover| cover.contains(pos))
        });
        placed
            .into_iter()
            .filter(|(layer, _)| foreign.is_none_or(|foreign| *layer > foreign))
            .map(|(_, node)| node)
            .collect()
    })
}

pub fn reveal_node(node: NodeId) {
    with_document(|document| document.reveal_node(node));
}

pub fn first_offset_within(node: NodeId) -> Option<NodeId> {
    with_document(|document| match document.contains(node) {
        true => document.first_offset_within(node).map(NodeOf::id),
        false => None,
    })
}

fn offset(document: &Document, node: NodeId) -> Option<NodeOf<OffsetNode>> {
    document.arena.kind_of::<OffsetNode>(node)
}

pub fn offset_value(node: NodeId) -> Option<f32> {
    with_document(|document| Some(document.offset_value(offset(document, node)?)))
}

pub fn offset_position(node: NodeId) -> Option<ScrollPosition> {
    with_document(|document| document.offset_position(offset(document, node)?))
}

pub fn offset_overscroll(node: NodeId) -> f32 {
    with_document(|document| match document.contains(node) {
        true => document.scroll_overscroll(node),
        false => 0.0,
    })
}

pub fn drive_offset(node: NodeId, value: f32) {
    with_document(|document| {
        if let Some(offset) = offset(document, node) {
            document.drive_offset(offset, value);
        }
    });
}

pub fn set_offset_overscroll(node: NodeId, overscroll: f32) {
    with_document(|document| {
        if let Some(offset) = offset(document, node) {
            document.set_offset_overscroll(offset, overscroll);
        }
    });
}

pub fn on_offset_steer(node: NodeId, handler: impl FnMut(()) + 'static) {
    with_document(|document| {
        if let Some(offset) = offset(document, node) {
            document.set_offset_on_steer(offset, handler);
        }
    });
}

pub fn virtual_list_offset<K: Clone + Hash + Eq + 'static>(list: NodeId, key: &K) -> Option<f32> {
    try_with_document(|document| document.virtual_list_offset::<K>(list, key)).flatten()
}

fn text_node(document: &Document, node: NodeId) -> Option<NodeOf<TextNode>> {
    document.arena.kind_of::<TextNode>(node)
}

pub fn texts_within(node: NodeId) -> Vec<NodeId> {
    with_document(|document| document.texts_within(node))
        .into_iter()
        .map(NodeOf::id)
        .collect()
}

pub fn text_content(node: NodeId) -> String {
    with_document(|document| {
        text_node(document, node).map_or_else(String::new, |text| document.text(text).to_owned())
    })
}

pub fn text_index_at(node: NodeId, pos: Pos2) -> Option<usize> {
    with_document(|document| document.text_index_at(text_node(document, node)?, pos))
}

pub fn text_caret_rect(node: NodeId, index: usize, width: f32) -> Option<Rect> {
    with_document(|document| document.text_caret_rect(text_node(document, node)?, index, width))
}

pub fn text_geometry(node: NodeId) -> Option<TextGeometry> {
    with_document(|document| Some(document.text_geometry(text_node(document, node)?)))
}

pub fn set_text_selection(node: NodeId, selection: Option<(Range<usize>, Color32)>) {
    with_document(|document| {
        if let Some(text) = text_node(document, node) {
            document.set_text_selection(text, selection);
        }
    });
}

pub fn set_text_selection_handles(node: NodeId, handles: Vec<(usize, CaretHandle, Color32)>) {
    with_document(|document| {
        if let Some(text) = text_node(document, node) {
            document.set_text_selection_handles(text, handles);
        }
    });
}
