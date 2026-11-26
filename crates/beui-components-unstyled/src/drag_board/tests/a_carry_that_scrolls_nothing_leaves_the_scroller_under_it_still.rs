use super::*;
use beui_view::drag::Carry;
use beui_view::types::input::PointerId;

#[test]
fn a_carry_that_scrolls_nothing_leaves_the_scroller_under_it_still() {
    let fixture = Fixture::new();
    fixture.scroller("form", rect(0.0, 0.0, 200.0, 200.0), 1);
    fixture.board.lift(
        Carry::new(1_u32).scrolls_nothing(),
        at_point(pos2(100.0, 100.0)),
    );
    let order = |_| (vec![1], WindowId::MAIN);
    Driver::track(&fixture.board, at_point(pos2(100.0, 190.0)), &order);
    Driver::release(&fixture.board, PointerId::Mouse, false, &order);
    assert_eq!(
        fixture.log.take(),
        Vec::<String>::new(),
        "a carry held in a scroller's edge band scrolls it only if it scrolls"
    );
}
