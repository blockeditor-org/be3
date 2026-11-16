use super::*;
use beui_view::drag::Carry;
use beui_view::types::input::PointerId;

#[test]
fn a_lifted_carry_scrolls_the_scroller_whose_edge_holds_it() {
    let fixture = Fixture::new();
    fixture.scroller("list", rect(0.0, 0.0, 200.0, 200.0), 1);
    fixture
        .board
        .lift(Carry::new(1_u32), at_point(pos2(100.0, 100.0)));
    fixture.log.take();
    let flat = Rc::clone(&fixture.flat);
    let order = move |node: NodeId| {
        let order = flat
            .rects
            .borrow()
            .get(&node)
            .map_or(0, |(_, order)| *order);
        (vec![order], WindowId::MAIN)
    };
    Driver::track(&fixture.board, at_point(pos2(100.0, 190.0)), &order);
    assert_eq!(
        fixture.log.take(),
        ["list scrolls Some(30.0)"],
        "a carry held in a scroller's edge band scrolls it"
    );
    Driver::release(&fixture.board, PointerId::Mouse, false, &order);
    assert_eq!(
        fixture.log.take(),
        ["list scrolls None"],
        "and the drop stops it"
    );
}
