use super::*;

#[test]
fn the_first_point_floats_the_item_unless_its_target_hides_the_preview() {
    let fixture = Fixture::new();
    fixture
        .target("list", rect(0.0, 0.0, 100.0, 100.0), 1)
        .hides_preview()
        .register();
    fixture.begin(pos2(50.0, 50.0));
    assert!(fixture.log.take().contains(&"floating false".to_owned()));
    fixture.board.track(at_point(pos2(60.0, 50.0)));
    assert!(!fixture.log.take().iter().any(|line| line.starts_with("floating")));
    fixture.board.track(at_point(pos2(150.0, 50.0)));
    assert!(fixture.log.take().contains(&"floating true".to_owned()));
    fixture.board.cancel();

    let flicked = Fixture::new();
    flicked
        .target("list", rect(0.0, 0.0, 100.0, 100.0), 1)
        .hides_preview()
        .register();
    flicked.begin(pos2(150.0, 50.0));
    assert!(flicked.log.take().contains(&"floating true".to_owned()));
}
