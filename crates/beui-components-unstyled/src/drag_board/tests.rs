use be_lib_stable_hash::HashMap;

use super::*;
use beui_view::reactive::{NodeRef, Scope};
use beui_view::types::{Modifiers, Pos2, pos2, vec2};

#[derive(Default)]
struct Flat {
    rects: RefCell<HashMap<NodeId, (Rect, usize)>>,
}

impl Space for Flat {
    fn placed(&self, node: NodeId) -> Option<(Rect, f32)> {
        Some((self.rects.borrow().get(&node)?.0, 1.0))
    }

    fn visible(&self, node: NodeId) -> Option<Rect> {
        Some(self.rects.borrow().get(&node)?.0)
    }
}

#[derive(Default)]
struct Log(RefCell<Vec<String>>);

impl Log {
    fn push(&self, line: impl Into<String>) {
        self.0.borrow_mut().push(line.into());
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.0.borrow_mut())
    }
}

struct Fixture {
    scope: Scope,
    flat: Rc<Flat>,
    board: Board,
    log: Rc<Log>,
    next: std::cell::Cell<u32>,
}

impl Fixture {
    fn new() -> Self {
        let flat = Rc::new(Flat::default());
        Self {
            scope: Scope::root(),
            board: Board::new(Rc::clone(&flat) as Rc<dyn Space>),
            flat,
            log: Rc::default(),
            next: std::cell::Cell::new(1),
        }
    }

    fn sync(&self) {
        let flat = Rc::clone(&self.flat);
        self.board.note_order(&move |node| {
            let order = flat
                .rects
                .borrow()
                .get(&node)
                .map_or(0, |(_, order)| *order);
            (vec![order], WindowId::MAIN)
        });
    }

    fn node(&self, rect: Rect, order: usize) -> NodeRef {
        let id = self.next.get();
        self.next.set(id + 1);
        let node = NodeId::new(id, 0);
        self.flat.rects.borrow_mut().insert(node, (rect, order));
        self.scope.run(|| {
            let node_ref = NodeRef::new();
            node_ref.fill(node);
            node_ref
        })
    }

    fn scroller(&self, name: &'static str, rect: Rect, order: usize) -> u64 {
        let log = Rc::clone(&self.log);
        let id = self.board.register_scroller(EdgeScroller {
            id: 0,
            node: self.node(rect, order),
            movable: Rc::new(|| [[false, false], [true, true]]),
            scroll: Rc::new(move |depth: Option<Vec2>| {
                log.push(format!("{name} scrolls {:?}", depth.map(|depth| depth.y)))
            }),
        });
        self.sync();
        id
    }

    /// The core's tracking `pos`: the board follows the carry of the pointer there.
    fn track(&self, pos: Pos2) {
        let flat = Rc::clone(&self.flat);
        let order = move |node: NodeId| {
            let order = flat
                .rects
                .borrow()
                .get(&node)
                .map_or(0, |(_, order)| *order);
            (vec![order], WindowId::MAIN)
        };
        Driver::track(&self.board, at_point(pos), &order);
    }
}

fn at_point(pos: Pos2) -> DragPoint {
    DragPoint::at(pos, Modifiers::NONE)
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(width, height))
}

mod a_carry_that_scrolls_nothing_leaves_the_scroller_under_it_still;
mod a_lifted_carry_scrolls_the_scroller_whose_edge_holds_it;
mod edge_scrolling_runs_the_topmost_scroller_whose_band_holds_the_pointer;
