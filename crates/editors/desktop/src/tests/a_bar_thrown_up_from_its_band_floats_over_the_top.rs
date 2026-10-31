use block_editor_beui::beui::{Pos2, pos2, vec2};

use super::*;

const DISPLAY: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(960.0, 640.0));

#[test]
fn a_bar_thrown_up_from_its_band_floats_over_the_top() {
    let mut fixture = Fixture::new();
    fixture.report_screens(vec![monitor("left", DISPLAY, 1.0)]);
    fixture.test.show_on_displays(&[(DISPLAY, 1.0)]);
    fixture.settle();
    let grip = format!("dock.bar.{}.grip", SHELL_BARS + STATUS_BAR);
    let from = fixture.test.desk_rects_of(&grip)[0].center();
    let start = pos2(from.x, DISPLAY.center().y + 60.0);

    fixture.test.desk_press(from);
    fixture.test.desk_move(from + vec2(0.0, -20.0));
    fixture.test.desk_move(start);
    fixture.test.advance(std::time::Duration::from_millis(200));
    let mut at: Pos2 = start;
    for _ in 0..4 {
        at += vec2(-10.0, -40.0);
        fixture.test.desk_move(at);
    }
    fixture.test.desk_release(at);
    fixture.test.settle();
    let landed = fixture.test.desk_rects_of("desktop.bar.status");
    assert!(
        matches!(landed[..], [landed] if landed.top() < 24.0 && landed.width() > landed.height()),
        "thrown upwards from the middle, the bar floats over the top rather than docking: {landed:?}"
    );
}
