use std::hash::Hash;
use std::ops::Range;
use std::rc::Rc;

use beui_core::base::offset::{OffsetNode, ScrollPosition};
use beui_core::base::overlay::OverlayNode;
use beui_core::base::text::{TextGeometry, TextNode};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::drag_board::Board;
use beui_core::geometry::{Pos2, Rect};
use beui_core::layer::{Dismiss, LayerPolicy};
use beui_core::node::{NodeId, NodeOf, Rects};
use beui_core::preferences::{PreferenceLayer, PreferenceLayers, PreferenceSource, Preferences};
use beui_core::rich::CaretHandle;
use beui_core::window::WindowId;
use beui_tree::reactive::{Binding, Callback, Get, Memo, ReadSignal, create_memo};

use beui_core::current::{try_with_document, with_document};

pub trait Inspect {
    fn api<A: Clone + 'static>(&self, node: NodeId) -> A;

    fn node_rect(&self, node: NodeId) -> Option<Rect>;

    fn is_overlay_open(&self, node: NodeId) -> bool;
}

impl Inspect for Document {
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
    fn text_index_at(&self, pos: Pos2) -> Option<usize>;
    fn caret_rect(&self, index: usize, width: f32) -> Option<Rect>;
    fn text_geometry(&self) -> Option<TextGeometry>;
    fn set_text_selection(&self, selection: Option<(Range<usize>, Color32)>);
    fn set_text_selection_handles(&self, handles: Vec<(usize, CaretHandle, Color32)>);
}

fn offset_node(document: &Document, node: NodeId) -> Option<NodeOf<OffsetNode>> {
    document.arena.kind_of::<OffsetNode>(node)
}

fn text_node(document: &Document, node: NodeId) -> Option<NodeOf<TextNode>> {
    document.arena.kind_of::<TextNode>(node)
}

impl TreeNode for NodeId {
    fn exists(&self) -> bool {
        with_document(|document| document.contains(*self))
    }

    fn children(&self) -> Vec<NodeId> {
        with_document(|document| match document.contains(*self) {
            true => document.children(*self),
            false => Vec::new(),
        })
    }

    fn focus(&self) {
        with_document(|document| document.focus_focusable(*self));
    }

    fn focus_into(&self) {
        with_document(|document| document.focus_into(*self));
    }

    fn focus_is_within(&self) -> bool {
        with_document(|document| document.focus_is_within(*self))
    }

    fn focus_pending_within(&self) -> bool {
        with_document(|document| document.focus_pending_within(*self))
    }

    fn focusables_within(&self) -> Vec<NodeId> {
        with_document(|document| document.focusables_within(*self))
    }

    fn raise_overlay(&self) {
        with_document(|document| document.raise_overlay(*self));
    }

    fn dismiss_overlay(&self, reason: Dismiss) {
        with_document(|document| document.dismiss_overlay(*self, reason));
    }

    fn close_overlay(&self) {
        with_document(|document| {
            if let Some(overlay) = document.arena.kind_of::<OverlayNode>(*self) {
                document.close_overlay(overlay);
            }
        });
    }

    fn is_overlay_open(&self) -> bool {
        with_document(|document| document.is_overlay_open(*self))
    }

    fn reveal(&self) {
        with_document(|document| document.reveal_node(*self));
    }

    fn first_offset_within(&self) -> Option<NodeId> {
        with_document(|document| match document.contains(*self) {
            true => document.first_offset_within(*self).map(NodeOf::id),
            false => None,
        })
    }

    fn offset(&self) -> Option<f32> {
        with_document(|document| Some(document.offset_value(offset_node(document, *self)?)))
    }

    fn scroll_position(&self) -> Option<ScrollPosition> {
        with_document(|document| document.offset_position(offset_node(document, *self)?))
    }

    fn drive_offset(&self, value: f32) {
        with_document(|document| {
            if let Some(offset) = offset_node(document, *self) {
                document.drive_offset(offset, value);
            }
        });
    }

    fn offset_to_end(&self) {
        with_document(|document| {
            if let Some(offset) = offset_node(document, *self) {
                document.offset_to_end(offset);
            }
        });
    }

    fn on_steer(&self, handler: impl FnMut(()) + 'static) {
        with_document(|document| {
            if let Some(offset) = offset_node(document, *self) {
                document.set_offset_on_steer(offset, handler);
            }
        });
    }

    fn row_offset<K: Clone + Hash + Eq + 'static>(&self, key: &K) -> Option<f32> {
        try_with_document(|document| document.virtual_list_offset::<K>(*self, key)).flatten()
    }

    fn texts_within(&self) -> Vec<NodeId> {
        with_document(|document| document.texts_within(*self))
            .into_iter()
            .map(NodeOf::id)
            .collect()
    }

    fn text(&self) -> String {
        with_document(|document| {
            text_node(document, *self)
                .map_or_else(String::new, |text| document.text(text).to_owned())
        })
    }

    fn text_index_at(&self, pos: Pos2) -> Option<usize> {
        with_document(|document| document.text_index_at(text_node(document, *self)?, pos))
    }

    fn caret_rect(&self, index: usize, width: f32) -> Option<Rect> {
        with_document(|document| {
            document.text_caret_rect(text_node(document, *self)?, index, width)
        })
    }

    fn text_geometry(&self) -> Option<TextGeometry> {
        with_document(|document| Some(document.text_geometry(text_node(document, *self)?)))
    }

    fn set_text_selection(&self, selection: Option<(Range<usize>, Color32)>) {
        with_document(|document| {
            if let Some(text) = text_node(document, *self) {
                document.set_text_selection(text, selection);
            }
        });
    }

    fn set_text_selection_handles(&self, handles: Vec<(usize, CaretHandle, Color32)>) {
        with_document(|document| {
            if let Some(text) = text_node(document, *self) {
                document.set_text_selection_handles(text, handles);
            }
        });
    }
}

#[derive(Clone)]
pub struct Placements(Rc<Rects>);

impl Placements {
    pub fn rect(&self, node: NodeId) -> Option<Rect> {
        self.0.get(&node)
    }

    pub fn scale(&self, node: NodeId) -> f32 {
        self.0.scale_of(&node)
    }
}

pub fn placements() -> Placements {
    Placements(with_document(|document| Rc::clone(&document.rects)))
}

pub fn viewport_rect() -> Rect {
    with_document(|document| document.viewport_rect())
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

pub fn drag_board() -> Rc<Board> {
    with_document(|document| document.drag_board())
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
