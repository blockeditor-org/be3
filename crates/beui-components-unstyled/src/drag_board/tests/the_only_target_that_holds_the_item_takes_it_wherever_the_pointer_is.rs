use super::*;

#[test]
fn the_only_target_that_holds_the_item_takes_it_wherever_the_pointer_is() {
    let fixture = Fixture::new();
    fixture
        .target("home", rect(0.0, 0.0, 10.0, 10.0), 1)
        .holds()
        .register();
    fixture.board.set_outside(true);
    fixture.begin(pos2(500.0, 400.0));
    assert!(fixture.log.take().contains(&"home over 500 400".to_owned()));
    fixture.board.finish();
    let log = fixture.log.take();
    assert!(log.contains(&"home dropped at 500 400".to_owned()));
    assert!(!log.iter().any(|line| line.starts_with("outside")));

    let shared = Fixture::new();
    shared
        .target("home", rect(0.0, 0.0, 10.0, 10.0), 1)
        .holds()
        .register();
    shared
        .target("other", rect(20.0, 0.0, 10.0, 10.0), 1)
        .register();
    shared.begin(pos2(500.0, 400.0));
    assert!(!shared.log.take().iter().any(|line| line.contains("over")));
}
