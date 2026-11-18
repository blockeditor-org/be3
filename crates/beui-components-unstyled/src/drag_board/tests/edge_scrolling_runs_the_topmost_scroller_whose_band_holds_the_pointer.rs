use super::*;
use beui_view::drag::Carry;

#[test]
fn edge_scrolling_runs_the_topmost_scroller_whose_band_holds_the_pointer() {
    let fixture = Fixture::new();
    fixture.scroller("outer", rect(0.0, 0.0, 200.0, 400.0), 1);
    fixture.scroller("inner", rect(0.0, 0.0, 200.0, 200.0), 2);
    fixture
        .board
        .lift(Carry::new(1_u32), at_point(pos2(100.0, 100.0)));
    fixture.track(pos2(100.0, 190.0));
    assert_eq!(fixture.log.take(), ["inner scrolls Some(30.0)"]);
    fixture.track(pos2(100.0, 380.0));
    assert_eq!(
        fixture.log.take(),
        ["inner scrolls None", "outer scrolls Some(20.0)"]
    );
    fixture.board.cancel();
    assert_eq!(fixture.log.take(), ["outer scrolls None"]);
}
