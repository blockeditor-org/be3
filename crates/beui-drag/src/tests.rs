use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::*;
use beui_types::input::Modifiers;
use beui_types::pos2;

#[derive(Default)]
struct Flat {
    rects: RefCell<HashMap<NodeId, (Rect, u32)>>,
}

impl Surface for Flat {
    type Order = u32;

    fn placed(&self, node: NodeId) -> Option<(Rect, f32)> {
        Some((self.rects.borrow().get(&node)?.0, 1.0))
    }

    fn visible(&self, node: NodeId) -> Option<Rect> {
        Some(self.rects.borrow().get(&node)?.0)
    }

    fn window(&self, _node: NodeId) -> WindowId {
        WindowId::MAIN
    }

    fn target_order(&self, node: NodeId) -> u32 {
        self.rects.borrow().get(&node).map_or(0, |(_, order)| *order)
    }

    fn scroller_order(&self, node: NodeId, _visible: Rect) -> u32 {
        self.target_order(node)
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
    scope: beui_tree::reactive::Scope,
    board: Board<Flat>,
    log: Rc<Log>,
    next: std::cell::Cell<u32>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            scope: beui_tree::reactive::Scope::root(),
            board: Board::new(Flat::default()),
            log: Rc::default(),
            next: std::cell::Cell::new(1),
        }
    }

    fn node(&self, rect: Rect, order: u32) -> NodeRef {
        let id = self.next.get();
        self.next.set(id + 1);
        let node = NodeId::new(id, 0);
        self.board
            .surface()
            .rects
            .borrow_mut()
            .insert(node, (rect, order));
        self.scope.run(|| {
            let node_ref = NodeRef::new();
            node_ref.fill(node);
            node_ref
        })
    }

    fn target(&self, name: &'static str, rect: Rect, order: u32) -> TargetBuilder<'_> {
        TargetBuilder {
            fixture: self,
            name,
            node: self.node(rect, order),
            takes: Rc::new(|_| true),
            hides_preview: false,
            holds: false,
        }
    }

    fn scroller(&self, name: &'static str, rect: Rect, order: u32) -> u64 {
        let log = Rc::clone(&self.log);
        self.board.register_scroller(EdgeScroller {
            id: 0,
            node: self.node(rect, order),
            axis: Rc::new(|| Direction::Vertical),
            movable: Rc::new(|| [true, true]),
            scroll: Rc::new(move |depth| log.push(format!("{name} scrolls {depth:?}"))),
        })
    }

    fn begin(&self, from: Pos2) {
        let log = Rc::clone(&self.log);
        let floating = Rc::clone(&self.log);
        let cancelled = Rc::clone(&self.log);
        let dropped = Rc::clone(&self.log);
        let outside = Rc::clone(&self.log);
        self.board.begin(
            DragSource {
                item: Rect::from_min_max(pos2(-5.0, -5.0), pos2(5.0, 5.0)),
                follow: Rc::new(move |point: DragPoint| {
                    log.push(format!("follows {}", at(point.pos)));
                }),
                floating: Rc::new(move |on| floating.push(format!("floating {on}"))),
                cancelled: Rc::new(move || cancelled.push("cancelled")),
                dropped: Rc::new(move || dropped.push("source dropped")),
                outside: Rc::new(move |point: DragPoint| {
                    outside.push(format!("outside at {}", at(point.pos)));
                }),
                ..DragSource::new(Rc::new("card"))
            },
            at_point(from),
        );
    }
}

struct TargetBuilder<'a> {
    fixture: &'a Fixture,
    name: &'static str,
    node: NodeRef,
    takes: Rc<dyn Fn(DragPoint) -> bool>,
    hides_preview: bool,
    holds: bool,
}

impl TargetBuilder<'_> {
    fn declines(self) -> Self {
        Self {
            takes: Rc::new(|_| false),
            ..self
        }
    }

    fn hides_preview(self) -> Self {
        Self {
            hides_preview: true,
            ..self
        }
    }

    fn holds(self) -> Self {
        Self {
            holds: true,
            ..self
        }
    }

    fn register(self) -> u64 {
        let name = self.name;
        let (carrying, over, dropped) = (
            Rc::clone(&self.fixture.log),
            Rc::clone(&self.fixture.log),
            Rc::clone(&self.fixture.log),
        );
        let (hides, holds) = (self.hides_preview, self.holds);
        self.fixture.board.register(Target {
            id: 0,
            node: self.node,
            accepts: Rc::new(|payload| payload.is::<&str>()),
            takes: self.takes,
            carrying: Rc::new(move |on| carrying.push(format!("{name} carrying {on}"))),
            over: Rc::new(move |point: Option<(&dyn Any, DragPoint)>| {
                over.push(match point {
                    Some((_, point)) => format!("{name} over {}", at(point.local)),
                    None => format!("{name} left"),
                });
            }),
            dropped: Rc::new(move |_, point: DragPoint| {
                dropped.push(format!("{name} dropped at {}", at(point.local)));
            }),
            hides_preview: Rc::new(move || hides),
            holds: Rc::new(move |_| holds),
        })
    }
}

fn at(pos: Pos2) -> String {
    format!("{} {}", pos.x, pos.y)
}

fn at_point(pos: Pos2) -> DragPoint {
    DragPoint::at(pos, Modifiers::NONE)
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), beui_types::vec2(width, height))
}

mod a_carried_item_drops_on_the_topmost_target_under_the_pointer;
mod a_release_over_no_target_cancels_the_drag;
mod a_target_that_declines_the_drop_lets_the_one_beneath_take_it;
mod edge_scrolling_runs_the_topmost_scroller_whose_band_holds_the_pointer;
mod escape_cancels_a_drag_over_a_target_without_dropping_it;
mod released_outside_the_window_the_item_goes_to_the_sources_outside;
mod the_first_point_floats_the_item_unless_its_target_hides_the_preview;
mod the_only_target_that_holds_the_item_takes_it_wherever_the_pointer_is;
