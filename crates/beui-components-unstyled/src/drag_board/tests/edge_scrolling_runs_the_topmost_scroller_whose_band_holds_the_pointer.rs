use super::*;

#[test]
fn edge_scrolling_runs_the_topmost_scroller_whose_band_holds_the_pointer() {
    let fixture = Fixture::new();
    fixture.scroller("outer", rect(0.0, 0.0, 200.0, 400.0), 1);
    fixture.scroller("inner", rect(0.0, 0.0, 200.0, 200.0), 2);
    fixture.begin(pos2(100.0, 100.0));
    fixture.log.take();
    fixture.board.track(at_point(pos2(100.0, 190.0)));
    assert_eq!(
        fixture.log.take(),
        ["follows 100 190", "inner scrolls Some(30.0)"]
    );
    fixture.board.track(at_point(pos2(100.0, 380.0)));
    assert_eq!(
        fixture.log.take(),
        [
            "follows 100 380",
            "inner scrolls None",
            "outer scrolls Some(20.0)"
        ]
    );
    fixture.board.end(None);
    assert_eq!(fixture.log.take(), ["outer scrolls None", "cancelled"]);
}
