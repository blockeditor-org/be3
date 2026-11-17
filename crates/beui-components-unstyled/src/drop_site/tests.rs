use std::cell::RefCell;

use be_lib_stable_hash::HashMap;
use beui_view::drag::{Carry, DragPoint, Driver, closest, fixed, floating};
use beui_view::types::input::PointerId;
use beui_view::types::{Modifiers, WindowId, pos2, vec2};

use super::*;

mod a_carry_let_go_outside_every_window_tells_its_source_where;
mod a_lifted_item_is_offered_to_every_site_that_takes_it_and_dropped_on_the_one_chosen;
mod a_lists_slots_rest_where_the_carried_row_would_land_among_rows_of_any_length;
mod a_place_in_another_window_is_never_chosen_by_the_pointer;
mod a_site_gone_mid_carry_is_never_dropped_on;
mod a_site_hears_of_the_place_chosen_in_it_only_when_it_changes;
mod an_item_from_another_list_gets_a_place_at_each_gap_its_size;
mod another_pointer_neither_moves_nor_drops_a_carry;
mod two_sites_that_float_when_far_leave_the_item_to_the_near_ones_place;

/// Nodes where tests put them, at scale 1.
#[derive(Default)]
struct Flat(RefCell<HashMap<NodeId, Rect>>);

impl Space for Flat {
    fn placed(&self, node: NodeId) -> Option<(Rect, f32)> {
        Some((*self.0.borrow().get(&node)?, 1.0))
    }

    fn visible(&self, node: NodeId) -> Option<Rect> {
        self.0.borrow().get(&node).copied()
    }
}

struct Fixture {
    _root: RootScope,
    flat: Rc<Flat>,
    board: Rc<Board>,
    next: std::cell::Cell<u32>,
}

impl Fixture {
    fn new() -> Self {
        let root = RootScope::new();
        let flat = Rc::new(Flat::default());
        Self {
            _root: root,
            board: Rc::new(Board::new(Rc::clone(&flat) as Rc<dyn Space>)),
            flat,
            next: std::cell::Cell::new(1),
        }
    }

    fn node(&self, rect: Rect) -> (NodeId, NodeRef) {
        let id = self.next.get();
        self.next.set(id + 1);
        let node = NodeId::new(id, 0);
        self.flat.0.borrow_mut().insert(node, rect);
        let node_ref = NodeRef::new();
        node_ref.fill(node);
        (node, node_ref)
    }

    fn track(&self, pos: Pos2) {
        Driver::track(&*self.board, at(pos), &|_| (Vec::new(), WindowId::MAIN));
    }

    fn carried(&self) -> Option<u32> {
        self.board
            .carried()
            .and_then(|payload| payload.downcast_ref::<u32>().copied())
    }

    fn release(&self) {
        Driver::release(&*self.board, PointerId::Mouse, false, &|_| {
            (Vec::new(), WindowId::MAIN)
        });
    }
}

/// A 20-point item held by its centre.
fn at(pos: Pos2) -> DragPoint {
    DragPoint::at(pos, Modifiers::NONE)
}

fn carry<P: 'static>(payload: P) -> Carry {
    Carry::new(payload).item(Rect::from_min_size(pos2(-10.0, -10.0), vec2(20.0, 20.0)))
}
