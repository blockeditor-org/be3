use be_sdk_beui::beui::pos2;

use super::*;

const FIRST: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0));
const SECOND: Rect = Rect::from_min_max(pos2(800.0, 0.0), pos2(1600.0, 600.0));
const THIRD: Rect = Rect::from_min_max(pos2(1600.0, 0.0), pos2(2400.0, 600.0));

fn status_down_the_left(fixture: &Fixture, on: Rect) -> bool {
    matches!(
        fixture.shown_on("desktop.bar.status", on)[..],
        [bar] if bar.left() == on.left() && bar.height() > bar.width()
    )
}

#[test]
fn a_display_keeps_its_own_bars_when_the_displays_change_places() {
    let mut fixture = Fixture::new();
    fixture.on_monitors(&[("built-in", FIRST), ("dell", SECOND), ("lg", THIRD)]);
    fixture.display_bar_menu(1, STATUS_BAR, SECOND, &["beside.ends", "beside.left.top"]);
    assert!(
        status_down_the_left(&fixture, SECOND) && !status_down_the_left(&fixture, THIRD),
        "the Dell's Status bar runs down its left edge, the LG's stays along its foot"
    );

    fixture.on_monitors(&[("built-in", FIRST), ("lg", SECOND), ("dell", THIRD)]);
    assert!(
        status_down_the_left(&fixture, THIRD),
        "the Dell's bars follow it to the third place"
    );
    assert!(
        !status_down_the_left(&fixture, SECOND),
        "the LG in the second place keeps its own bars, not the Dell's"
    );

    let saved = fixture.saved();
    let mut fixture = Fixture::resumed(saved.clone());
    fixture.on_monitors(&[("built-in", FIRST), ("lg", SECOND)]);
    assert!(
        !status_down_the_left(&fixture, SECOND),
        "with the Dell unplugged, the LG does not take its bars"
    );

    let mut fixture = Fixture::resumed(saved);
    fixture.on_monitors(&[("built-in", FIRST), ("dell", SECOND), ("lg", THIRD)]);
    assert!(
        status_down_the_left(&fixture, SECOND) && !status_down_the_left(&fixture, THIRD),
        "a desktop started again gives each display the bars it saved"
    );
}
