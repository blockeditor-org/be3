use super::*;

#[test]
fn a_carried_item_drops_on_the_topmost_target_under_the_pointer() {
    let fixture = Fixture::new();
    fixture
        .target("pane", rect(0.0, 0.0, 100.0, 100.0), 2)
        .register();
    fixture
        .target("bar", rect(10.0, 0.0, 90.0, 20.0), 3)
        .register();
    fixture
        .target("hidden", rect(0.0, 0.0, 100.0, 20.0), 1)
        .register();
    fixture.begin(pos2(50.0, 50.0));
    assert_eq!(
        fixture.log.take(),
        [
            "pane carrying true",
            "bar carrying true",
            "hidden carrying true",
            "follows 50 50",
            "floating true",
            "pane over 50 50",
        ]
    );
    fixture.board.track(at_point(pos2(30.0, 10.0)));
    assert_eq!(
        fixture.log.take(),
        ["follows 30 10", "pane left", "bar over 20 10"]
    );
    assert!(fixture.board.end(Some(at_point(pos2(30.0, 12.0)))));
    let log = fixture.log.take();
    assert_eq!(log.first().map(String::as_str), Some("bar over 20 12"));
    assert!(log.contains(&"bar dropped at 20 12".to_owned()));
    assert_eq!(log.last().map(String::as_str), Some("source dropped"));
    assert!(!fixture.board.carrying());
}
