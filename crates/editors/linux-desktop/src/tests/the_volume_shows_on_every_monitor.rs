use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn the_volume_shows_on_every_monitor() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let right = Rect::from_min_size(pos2(800.0, 200.0), vec2(560.0, 400.0));
    let mut fixture = Fixture::new();
    fixture.report_screens(vec![
        monitor("left", left, 1.0),
        monitor("right", right, 2.0),
    ]);
    fixture.test.show_on_displays(&[(left, 1.0), (right, 2.0)]);
    fixture.test.set_watched::<MediaWatch>(&MediaLevels {
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
        fixture.test.desk_rects_of("desktop.osd.0"),
        fixture.test.desk_rects_of("desktop.osd.1"),
    ];
    assert!(
        shown[0].iter().all(|rect| left.contains_rect(*rect)) && shown[0].len() == 1,
        "the level shows on the first monitor the host reports: {shown:?}"
    );
    assert!(
        shown[1].iter().all(|rect| right.contains_rect(*rect)) && shown[1].len() == 1,
        "and on the second, in its own region: {shown:?}"
    );
    fixture.test.desk_snapshot("the_volume_on_two_monitors_left", 0);
    fixture.test.desk_snapshot("the_volume_on_two_monitors_right", 1);
}
