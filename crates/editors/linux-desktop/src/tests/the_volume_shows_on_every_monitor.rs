use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn the_volume_shows_on_every_monitor() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 600.0));
    let right = Rect::from_min_size(pos2(500.0, 0.0), vec2(300.0, 400.0));
    let mut fixture = Fixture::new();
    fixture
        .test
        .set_monitors(vec![("left", left), ("right", right)]);
    fixture.settle();
    fixture.test.set_host_value::<Media>(&MediaLevels {
        output: Some(MediaLevel {
            level: 0.4,
            muted: false,
        }),
        input: None,
        brightness: None,
        ..MediaLevels::default()
    });
    assert!(fixture.test.app_key(Modifiers::NONE, Key::VolumeUp));
    fixture.settle();
    let shown = [
        fixture.test.rect_of("desktop.osd.0"),
        fixture.test.rect_of("desktop.osd.1"),
    ];
    assert!(
        left.contains_rect(shown[0]),
        "the level shows on the first monitor the host reports: {shown:?}"
    );
    assert!(
        right.contains_rect(shown[1]),
        "and on the second, within it rather than across the seam: {shown:?}"
    );
    fixture.test.snapshot("the_volume_on_two_monitors");
}
