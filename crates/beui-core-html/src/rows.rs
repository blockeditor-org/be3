use beui_types::Direction;

use crate::document::{set_attribute, set_style};
use crate::node::NodeId;

pub fn spacer(direction: Direction, node: NodeId, length: f32) {
    set_attribute(node, "data-beui-spacer", Some(String::new()));
    let (main, cross) = match direction {
        Direction::Horizontal => ("width", "height"),
        Direction::Vertical => ("height", "width"),
    };
    set_style(node, "flex", Some("0 0 auto".to_owned()));
    set_style(node, cross, None);
    set_style(node, main, Some(crate::css::px(length.max(0.0))));
}

pub fn visible_span(list: NodeId, direction: Direction) -> Option<(f32, f32)> {
    #[cfg(target_arch = "wasm32")]
    {
        web::visible_span(list, direction)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (list, direction);
        None
    }
}

pub fn main_length(node: NodeId, direction: Direction) -> Option<f32> {
    #[cfg(target_arch = "wasm32")]
    {
        let element = crate::dom::element(node).filter(|element: &web_sys::Element| element.is_connected())?;
        let rect = crate::dom::client_rect(&element);
        let scale = crate::dom::scale_of(&element, rect);
        Some(direction.main(rect.size()) / scale)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (node, direction);
        None
    }
}

#[derive(Default)]
pub struct Watch {
    #[cfg(target_arch = "wasm32")]
    resizes: Option<crate::dom::Resizes>,
}

impl Watch {
    pub fn new() -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            resizes: crate::dom::Resizes::new(crate::frame::relayout),
        }
    }

    pub fn observe(&self, node: NodeId) {
        #[cfg(target_arch = "wasm32")]
        if let (Some(resizes), Some(element)) = (&self.resizes, crate::dom::element(node)) {
            resizes.observe(&element);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = node;
    }

    pub fn unobserve(&self, node: NodeId) {
        #[cfg(target_arch = "wasm32")]
        if let (Some(resizes), Some(element)) = (&self.resizes, crate::dom::element(node)) {
            resizes.unobserve(&element);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = node;
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use beui_types::{Direction, Rect};

    pub(super) fn visible_span(list: crate::node::NodeId, direction: Direction) -> Option<(f32, f32)> {
        let element = crate::dom::element(list).filter(|element: &web_sys::Element| element.is_connected())?;
        let rect = crate::dom::client_rect(&element);
        let mut clip = crate::dom::viewport();
        for viewer in crate::dom::nodes_above(element.parent_node()) {
            if crate::scroll::find(viewer).is_none() {
                continue;
            }
            if let Some(viewer) = crate::dom::element(viewer) {
                clip = clip.intersect(crate::dom::client_rect(&viewer));
            }
        }
        let scale = crate::dom::scale_of(&element, rect);
        let span = |rect: Rect| match direction {
            Direction::Horizontal => (rect.left(), rect.right()),
            Direction::Vertical => (rect.top(), rect.bottom()),
        };
        let (start, _) = span(rect);
        let (from, to) = span(clip);
        let first = ((from - start) / scale).max(0.0);
        let last = ((to - start) / scale).max(first);
        Some((first, last))
    }
}
