use super::*;

#[test]
fn a_target_that_declines_the_drop_lets_the_one_beneath_take_it() {
    let fixture = Fixture::new();
    fixture
        .target("pane", rect(0.0, 0.0, 100.0, 100.0), 1)
        .register();
    fixture
        .target("bar", rect(0.0, 0.0, 100.0, 20.0), 2)
        .declines()
        .register();
    fixture.begin(pos2(50.0, 10.0));
    assert!(fixture.log.take().contains(&"pane over 50 10".to_owned()));
    assert!(fixture.board.end(Some(at_point(pos2(50.0, 10.0)))));
    let log = fixture.log.take();
    assert!(log.contains(&"pane dropped at 50 10".to_owned()));
    assert!(!log.iter().any(|line| line.starts_with("bar dropped")));
}
