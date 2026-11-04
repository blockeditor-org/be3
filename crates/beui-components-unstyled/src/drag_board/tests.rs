use be_lib_stable_hash::HashMap;

use super::*;
use beui_view::reactive::{NodeRef, Scope};
use beui_view::types::{Direction, Modifiers, Pos2, pos2, vec2};

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

    fn target(&self, name: &'static str, rect: Rect, order: usize) -> TargetBuilder<'_> {
        TargetBuilder {
            fixture: self,
            name,
            node: self.node(rect, order),
            takes: Rc::new(|_| Take::Drop),
            hides_preview: false,
            holds: false,
        }
    }

    fn scroller(&self, name: &'static str, rect: Rect, order: usize) -> u64 {
        let log = Rc::clone(&self.log);
        let id = self.board.register_scroller(EdgeScroller {
            id: 0,
            node: self.node(rect, order),
            axis: Rc::new(|| Direction::Vertical),
            movable: Rc::new(|| [true, true]),
            scroll: Rc::new(move |depth| log.push(format!("{name} scrolls {depth:?}"))),
        });
        self.sync();
        id
    }

    fn begin(&self, from: Pos2) {
        let log = Rc::clone(&self.log);
        let floating = Rc::clone(&self.log);
        let cancelled = Rc::clone(&self.log);
        let dropped = Rc::clone(&self.log);
        let outside = Rc::clone(&self.log);
        self.sync();
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
    takes: Rc<dyn Fn(DragPoint) -> Take>,
    hides_preview: bool,
    holds: bool,
}

impl TargetBuilder<'_> {
    fn declines(self) -> Self {
        Self {
            takes: Rc::new(|_| Take::Pass),
            ..self
        }
    }

    fn refuses(self) -> Self {
        Self {
            takes: Rc::new(|_| Take::Refuse),
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
        let id = self.fixture.board.register(Target {
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
        });
        self.fixture.sync();
        id
    }
}

fn at(pos: Pos2) -> String {
    format!("{} {}", pos.x, pos.y)
}

fn at_point(pos: Pos2) -> DragPoint {
    DragPoint::at(pos, Modifiers::NONE)
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(width, height))
}

mod a_carried_item_drops_on_the_topmost_target_under_the_pointer;
mod a_release_over_no_target_cancels_the_drag;
mod a_target_that_declines_the_drop_lets_the_one_beneath_take_it;
mod a_target_that_refuses_the_drop_hears_the_item_over_it_and_sends_it_home;
mod edge_scrolling_runs_the_topmost_scroller_whose_band_holds_the_pointer;
mod escape_cancels_a_drag_over_a_target_without_dropping_it;
mod every_target_hears_the_drag_end_before_the_drop;
mod released_outside_the_window_the_item_goes_to_the_sources_outside;
mod the_first_point_floats_the_item_unless_its_target_hides_the_preview;
mod the_only_target_that_holds_the_item_takes_it_wherever_the_pointer_is;
