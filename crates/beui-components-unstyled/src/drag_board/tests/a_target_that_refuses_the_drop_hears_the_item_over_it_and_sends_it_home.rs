use super::*;

#[test]
fn a_target_that_refuses_the_drop_hears_the_item_over_it_and_sends_it_home() {
    let fixture = Fixture::new();
    fixture
        .target("pane", rect(0.0, 0.0, 100.0, 100.0), 1)
        .register();
    fixture
        .target("locked", rect(0.0, 0.0, 100.0, 50.0), 2)
        .refuses()
        .hides_preview()
        .register();
    fixture.begin(pos2(50.0, 10.0));
    let log = fixture.log.take();
    assert!(
        log.contains(&"locked over 50 10".to_owned()),
        "the refusing target hears the item over it, so it can show the refusal: {log:?}"
    );
    assert!(
        !log.iter().any(|line| line.starts_with("pane over")),
        "and the target beneath does not: {log:?}"
    );
    assert!(
        log.contains(&"floating true".to_owned()),
        "a target that refuses the item does not hide its preview: {log:?}"
    );
    assert!(!fixture.board.end(Some(at_point(pos2(50.0, 10.0)))));
    let log = fixture.log.take();
    assert!(
        !log.iter().any(|line| line.contains("dropped")),
        "let go there, nothing takes it: {log:?}"
    );
    assert!(
        log.contains(&"cancelled".to_owned()),
        "and the source takes it back: {log:?}"
    );
}
