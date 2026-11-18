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
        let element = crate::dom::element(node)
            .filter(|element: &web_sys::Element| element.is_connected())?;
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

/// Where `row` starts along `list`, in points.
pub fn start_within(list: NodeId, row: NodeId, direction: Direction) -> Option<f32> {
    #[cfg(target_arch = "wasm32")]
    {
        web::start_within(list, row, direction)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (list, row, direction);
        None
    }
}

/// Scrolls the scroll around `list` on to follow a row that moved `drift` points along it,
/// so the row stays where it is shown: unless the browser anchors that scroll itself
/// (`overflow-anchor: auto`, which Safari lacks) or it is an `Offset`, which anchors itself.
/// Only an element's own scroll (`overflow: auto` or `scroll`) is followed: a list scrolled by
/// the page itself (the document's scrolling element, whose overflow is `visible`) is not.
pub fn follow_row(list: NodeId, direction: Direction, drift: f32) {
    #[cfg(target_arch = "wasm32")]
    web::follow_row(list, direction, drift);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (list, direction, drift);
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

    pub(super) fn start_within(
        list: crate::node::NodeId,
        row: crate::node::NodeId,
        direction: Direction,
    ) -> Option<f32> {
        let connected = |node| {
            crate::dom::element(node).filter(|element: &web_sys::Element| element.is_connected())
        };
        let (list, row) = (connected(list)?, connected(row)?);
        let rect = crate::dom::client_rect(&list);
        let scale = crate::dom::scale_of(&list, rect);
        Some(direction.main(crate::dom::client_rect(&row).min - rect.min) / scale)
    }

    pub(super) fn follow_row(list: crate::node::NodeId, direction: Direction, drift: f32) {
        let Some(list) = crate::dom::element(list) else {
            return;
        };
        let Some(window) = web_sys::window() else {
            return;
        };
        let (overflow, position) = match direction {
            Direction::Horizontal => ("overflow-x", "scrollLeft"),
            Direction::Vertical => ("overflow-y", "scrollTop"),
        };
        let mut above = list.parent_element();
        while let Some(viewer) = above {
            above = viewer.parent_element();
            if crate::scroll::anchors(&viewer) {
                return;
            }
            let Some(style) = window.get_computed_style(&viewer).ok().flatten() else {
                continue;
            };
            let scrolls = style
                .get_property_value(overflow)
                .is_ok_and(|value| value == "auto" || value == "scroll");
            if !scrolls {
                continue;
            }
            // Where the browser anchors the scroll, it already followed the row.
            if style
                .get_property_value("overflow-anchor")
                .is_ok_and(|value| value == "auto")
            {
                return;
            }
            // `drift` is in the list's own CSS px (points), and `scrollTop` in the viewer's:
            // they differ by any transform between the two, so go through the screen's.
            let scale = |element: &web_sys::Element| {
                crate::dom::scale_of(element, crate::dom::client_rect(element))
            };
            let by = drift * scale(&list) / scale(&viewer);
            let at = js_sys::Reflect::get(&viewer, &position.into())
                .ok()
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0);
            let to = at + f64::from(by);
            let _ = js_sys::Reflect::set(&viewer, &position.into(), &to.into());
            return;
        }
    }

    pub(super) fn visible_span(
        list: crate::node::NodeId,
        direction: Direction,
    ) -> Option<(f32, f32)> {
        let element = crate::dom::element(list)
            .filter(|element: &web_sys::Element| element.is_connected())?;
        let rect = crate::dom::client_rect(&element);
        let mut clip = crate::dom::viewport();
        let mut above = element.parent_element();
        while let Some(viewer) = above {
            if crate::dom::clips(&viewer) {
                clip = clip.intersect(crate::dom::client_rect(&viewer));
            }
            above = viewer.parent_element();
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
