use std::rc::Rc;

use beui_core::current::{try_with_document, with_document};
use beui_core::node::Rects;
use beui_types::drag::{Driver, Space};
use beui_types::{NodeId, Rect, WindowId};

/// The document's nodes: where its last layout put them, and the window each is in now.
struct Placed(Rc<Rects>);

impl Space for Placed {
    fn placed(&self, node: NodeId) -> Option<(Rect, f32)> {
        self.0.scaled(&node)
    }

    fn visible(&self, node: NodeId) -> Option<Rect> {
        self.0.visible(&node)
    }

    fn window(&self, node: NodeId) -> Option<WindowId> {
        try_with_document(|document| document.contains(node).then(|| document.window_of(node)))
            .flatten()
    }
}

pub(crate) fn space() -> Rc<dyn Space> {
    with_document(|document| Rc::new(Placed(Rc::clone(&document.rects))) as Rc<dyn Space>)
}

/// None also with no document installed: a node torn down with its document asks too.
pub(crate) fn driver() -> Option<Rc<dyn Driver>> {
    beui_core::current::try_with_document(|document| document.drags.clone()).flatten()
}

pub(crate) fn drive(driver: Rc<dyn Driver>) {
    with_document(|document| document.drags = Some(driver));
}
