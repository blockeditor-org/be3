use std::cell::RefCell;
use std::rc::Rc;

use foldhash::HashMap;

use crate::document::Document;
use crate::geometry::Rect;
use crate::node::{NodeId, Rects};
use crate::window::WindowId;

pub use beui_drag::{
    Accepts, DragPoint, DragSource, Dropped, EDGE_BAND, EdgeScroller, Outside, Over, Pending,
    Surface, Target,
};

pub type Board = beui_drag::Board<Painted>;

pub struct Painted {
    rects: Rc<Rects>,
    order: RefCell<HashMap<NodeId, Vec<usize>>>,
    windows: RefCell<HashMap<NodeId, WindowId>>,
}

impl Painted {
    pub fn new(rects: Rc<Rects>) -> Self {
        Self {
            rects,
            order: RefCell::default(),
            windows: RefCell::default(),
        }
    }

    fn update(&self, doc: &Document, nodes: &[NodeId]) {
        *self.order.borrow_mut() = nodes
            .iter()
            .map(|node| (*node, doc.paint_order(*node)))
            .collect();
        *self.windows.borrow_mut() = nodes
            .iter()
            .map(|node| (*node, doc.window_of(*node)))
            .collect();
    }
}

impl Surface for Painted {
    type Order = Option<Vec<usize>>;

    fn placed(&self, node: NodeId) -> Option<(Rect, f32)> {
        self.rects.scaled(&node)
    }

    fn visible(&self, node: NodeId) -> Option<Rect> {
        self.rects.visible(&node)
    }

    fn window(&self, node: NodeId) -> WindowId {
        self.windows
            .borrow()
            .get(&node)
            .copied()
            .unwrap_or(WindowId::MAIN)
    }

    fn target_order(&self, node: NodeId) -> Self::Order {
        self.order.borrow().get(&node).cloned()
    }

    fn scroller_order(&self, node: NodeId, _visible: Rect) -> Self::Order {
        self.target_order(node)
    }
}

impl Document {
    pub fn drag_board(&self) -> Rc<Board> {
        Rc::clone(&self.drags)
    }

    pub fn dragging(&self) -> bool {
        self.drags.carrying()
    }

    pub fn order_drags(&self) {
        let nodes = self.drags.nodes();
        if nodes.is_empty() {
            return;
        }
        self.drags.surface().update(self, &nodes);
    }
}
