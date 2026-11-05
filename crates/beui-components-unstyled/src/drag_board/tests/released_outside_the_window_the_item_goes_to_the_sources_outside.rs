use super::*;

#[test]
fn released_outside_the_window_the_item_goes_to_the_sources_outside() {
    let fixture = Fixture::new();
    fixture
        .target("list", rect(0.0, 0.0, 100.0, 100.0), 1)
        .register();
    fixture.begin(pos2(50.0, 50.0));
    fixture.log.take();
    fixture.board.set_outside(true);
    fixture.board.track(at_point(pos2(300.0, 50.0)));
    assert_eq!(fixture.log.take(), ["list left"]);
    fixture.board.finish();
    let log = fixture.log.take();
    assert_eq!(log.last().map(String::as_str), Some("outside at 300 50"));
    assert!(!log.contains(&"cancelled".to_owned()));
    assert!(!fixture.board.carrying());
}
