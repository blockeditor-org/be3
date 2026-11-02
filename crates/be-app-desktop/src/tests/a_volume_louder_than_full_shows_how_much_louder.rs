use super::*;
use crate::root::osd::spans;

#[test]
fn a_volume_louder_than_full_shows_how_much_louder() {
    let close = |level: f32, expected: (f32, f32, f32)| {
        let (full, louder, rest) = spans(level);
        (full - expected.0).abs() < 1e-5
            && (louder - expected.1).abs() < 1e-5
            && (rest - expected.2).abs() < 1e-5
    };
    assert!(close(0.4, (0.4, 0.0, 0.6)));
    assert!(close(1.0, (1.0, 0.0, 0.0)));
    assert!(
        close(1.2, (1.0 / 1.5, 0.2 / 1.5, 0.2)),
        "past full the bar reaches to 150%, the part past full apart"
    );
    assert!(close(2.0, (0.5, 0.5, 0.0)));

    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(fixture.test.host_key(Modifiers::NONE, Key::VolumeUp));
    fixture.hear(1.5, false, SPEAKERS);
    assert!(fixture.test.shown("desktop.osd.0"));
    fixture.test.snapshot("the_volume_louder_than_full");
    assert!(
        fixture.status_label().contains("150%"),
        "{}",
        fixture.status_label()
    );
}
