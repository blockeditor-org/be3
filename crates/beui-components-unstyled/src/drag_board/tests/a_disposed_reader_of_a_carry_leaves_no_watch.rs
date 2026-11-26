use super::*;
use beui_view::drag::Carry;
use beui_view::reactive::{Scope, create_effect, flush};

fn watches(board: &Board) -> usize {
    board.watches.borrow().values().map(HashMap::len).sum()
}

/// A reader of a payload's carry holds one watch while it runs, and none once its scope is
/// disposed, however often a lift reran it.
#[test]
fn a_disposed_reader_of_a_carry_leaves_no_watch() {
    let fixture = Fixture::new();
    let board = Rc::new(fixture.board);
    let readers = Scope::new();
    readers.run(|| {
        for payload in [1_u32, 2_u32] {
            let board = Rc::clone(&board);
            create_effect(move || {
                board.carries(&payload);
            });
        }
    });
    flush();
    assert_eq!(watches(&board), 2, "each reader holds one watch");

    board.lift(Carry::new(1_u32), at_point(pos2(10.0, 10.0)));
    flush();
    assert_eq!(watches(&board), 2, "a reader rerun by a lift holds one watch still");

    readers.dispose();
    assert_eq!(watches(&board), 0, "a disposed reader leaves no watch behind");
}
