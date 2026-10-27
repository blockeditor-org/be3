use std::hash::Hash;
use std::ops::Range;

use beui_core::base::offset::{OffsetNode, ScrollPosition};
use beui_core::base::overlay::OverlayNode;
use beui_core::base::text::TextNode;
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Rect};
use beui_core::layer::{Dismiss, LayerPolicy};
use beui_core::node::{NodeId, NodeOf};
use beui_core::preferences::{PreferenceLayer, PreferenceLayers, PreferenceSource, Preferences};
use beui_core::rich::CaretHandle;
use beui_core::window::WindowId;
use beui_tree::reactive::{Binding, Callback, Get, Memo, ReadSignal, create_memo};

use beui_core::current::{try_with_document, with_document};

impl crate::nodes::Inspect for Document {
    fn api<A: Clone + 'static>(&self, node: NodeId) -> A {
        Document::api::<A>(self, node)
    }

    fn node_rect(&self, node: NodeId) -> Option<Rect> {
        Document::node_rect(self, node)
    }

    fn is_overlay_open(&self, node: NodeId) -> bool {
        Document::is_overlay_open(self, node)
    }
}

fn offset_node(document: &Document, node: NodeId) -> Option<NodeOf<OffsetNode>> {
    document.arena.kind_of::<OffsetNode>(node)
}

fn text_node(document: &Document, node: NodeId) -> Option<NodeOf<TextNode>> {
    document.arena.kind_of::<TextNode>(node)
}

pub fn exists(node: NodeId) -> bool {
    with_document(|document| document.contains(node))
}

pub fn children(node: NodeId) -> Vec<NodeId> {
    with_document(|document| match document.contains(node) {
        true => document.children(node),
        false => Vec::new(),
    })
}

pub fn focus(node: NodeId) {
    with_document(|document| document.focus_focusable(node));
}

pub fn focus_into(node: NodeId) {
    with_document(|document| document.focus_into(node));
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

pub fn raise_overlay(node: NodeId) {
    with_document(|document| document.raise_overlay(node));
}

pub fn dismiss_overlay(node: NodeId, reason: Dismiss) {
    with_document(|document| document.dismiss_overlay(node, reason));
}

pub fn close_overlay(node: NodeId) {
    with_document(|document| {
        if let Some(overlay) = document.arena.kind_of::<OverlayNode>(node) {
            document.close_overlay(overlay);
        }
    });
}

pub fn is_overlay_open(node: NodeId) -> bool {
    with_document(|document| document.is_overlay_open(node))
}

pub fn reveal(node: NodeId) {
    with_document(|document| document.reveal_node(node));
}

pub fn first_offset_within(node: NodeId) -> Option<NodeId> {
    with_document(|document| match document.contains(node) {
        true => document.first_offset_within(node).map(NodeOf::id),
        false => None,
    })
}

pub fn offset(node: NodeId) -> Option<f32> {
    with_document(|document| Some(document.offset_value(offset_node(document, node)?)))
}

pub fn scroll_position(node: NodeId) -> Option<ScrollPosition> {
    with_document(|document| document.offset_position(offset_node(document, node)?))
}

pub fn drive_offset(node: NodeId, value: f32) {
    with_document(|document| {
        if let Some(offset) = offset_node(document, node) {
            document.drive_offset(offset, value);
        }
    });
}

pub fn offset_to_end(node: NodeId) {
    with_document(|document| {
        if let Some(offset) = offset_node(document, node) {
            document.offset_to_end(offset);
        }
    });
}

pub fn on_steer(node: NodeId, handler: impl FnMut(()) + 'static) {
    with_document(|document| {
        if let Some(offset) = offset_node(document, node) {
            document.set_offset_on_steer(offset, handler);
        }
    });
}

pub fn row_offset<K: Clone + Hash + Eq + 'static>(node: NodeId, key: &K) -> Option<f32> {
    try_with_document(|document| document.virtual_list_offset::<K>(node, key)).flatten()
}

pub fn texts_within(node: NodeId) -> Vec<NodeId> {
    with_document(|document| document.texts_within(node))
        .into_iter()
        .map(NodeOf::id)
        .collect()
}

pub fn text(node: NodeId) -> String {
    with_document(|document| {
        text_node(document, node)
            .map_or_else(String::new, |text| document.text(text).to_owned())
    })
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

pub fn window_of(node: NodeId) -> WindowId {
    with_document(|document| document.window_of(node))
}

pub fn focused_node() -> Option<NodeId> {
    with_document(|document| document.focused_node())
}

pub fn set_focus(node: Option<NodeId>) {
    with_document(|document| document.update_focus(node));
}

pub fn focus_ancestry() -> Vec<NodeId> {
    with_document(|document| document.focus_ancestry())
}

pub fn stacked_at(nodes: &[NodeId], window: WindowId, pos: Pos2) -> Vec<NodeId> {
    with_document(|document| {
        let layers = document.layers();
        let mut placed: Vec<(usize, NodeId)> = nodes
            .iter()
            .filter(|node| document.window_of(**node) == window)
            .filter_map(|node| Some((document.layer_of(&layers, *node)?, *node)))
            .collect();
        placed.sort_by_key(|(layer, _)| std::cmp::Reverse(*layer));
        let foreign = (1..layers.len()).rev().find(|index| {
            layers[*index].window == window
                && layers[*index].policy != LayerPolicy::Inert
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

pub fn tree_local<T: Clone + 'static>(make: impl FnOnce() -> T) -> T {
    with_document(|document| document.extension_or_insert_with(make))
}


pub fn use_preferences() -> Memo<Preferences> {
    let (theme, motion, canvas_touch) = with_document(|document| {
        (
            document.watch_theme(),
            document.watch_motion(),
            document.watch_canvas_touch(),
        )
    });
    create_memo(move || Preferences {
        theme: theme.get(),
        motion: motion.get(),
        canvas_touch: canvas_touch.get(),
    })
}

pub fn use_preference_layers() -> ReadSignal<PreferenceLayers> {
    with_document(|document| document.watch_preference_layers())
}

pub fn use_preference_layer(source: PreferenceSource) -> Binding<PreferenceLayer> {
    let layers = use_preference_layers();
    Binding::new(
        create_memo(move || layers.get().layer(source)),
        Callback::new(move |layer: PreferenceLayer| {
            with_document(|document| {
                if let Some(context) = document.context() {
                    context.set_preference_layer(source, layer);
                }
            });
        }),
    )
}

pub fn use_font_revision() -> ReadSignal<u64> {
    with_document(|document| document.watch_context())
}
