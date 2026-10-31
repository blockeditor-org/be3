use block_editor_beui::beui::pos2;

use super::*;

const DISPLAY: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(960.0, 640.0));

#[test]
fn a_floating_bar_dragged_by_its_grip_follows_to_the_end_and_lands_where_let_go() {
    let mut fixture = Fixture::new();
    fixture.report_screens(vec![monitor("left", DISPLAY, 1.0)]);
    fixture.test.show_on_displays(&[(DISPLAY, 1.0)]);
    fixture.settle();
    fixture.bar_menu(APPS_BAR, &["over.ends", "over.bottom.left"]);
    let grip = format!("dock.bar.{}.grip", SHELL_BARS + APPS_BAR);
    let from = fixture.test.desk_rects_of(&grip)[0].center();

    let rest = pos2(DISPLAY.center().x, DISPLAY.top() + DISPLAY.height() * 0.3);
    fixture.test.desk_press(from);
    let steps = 24;
    for step in 1..=steps {
        let at = from + (rest - from) * (step as f32 / steps as f32);
        fixture.test.desk_move(at);
        if step < steps / 2 {
            continue;
        }
        let shown = fixture.test.desk_rects_of("desktop.bar.apps");
        assert!(
            matches!(shown[..], [shown] if shown.contains(at)),
            "step {step}: away from the edges it passed, the bar keeps under the pointer: {shown:?} for {at:?}"
        );
    }
    fixture.test.advance(std::time::Duration::from_millis(200));
    fixture.test.desk_release(rest);
    fixture.test.settle();
    let landed = fixture.test.desk_rects_of("desktop.bar.apps");
    assert!(
        matches!(landed[..], [landed] if landed.top() < 24.0
            && (landed.center().x - DISPLAY.center().x).abs() < 24.0),
        "let go at rest nearest the top, the bar floats over the middle of the top: {landed:?}"
    );
    fixture
        .test
        .desk_snapshot("a_floating_bar_dragged_to_float_over_the_top", 0);
}
