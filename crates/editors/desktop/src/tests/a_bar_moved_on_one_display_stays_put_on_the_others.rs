use block_editor_beui::beui::pos2;

use super::*;

const LEFT: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(960.0, 640.0));
const RIGHT: Rect = Rect::from_min_max(pos2(960.0, 160.0), pos2(1600.0, 640.0));

fn status_down_the_left_of_the_right_display_only(fixture: &Fixture) {
    let left = fixture.shown_on("desktop.bar.status", LEFT);
    let right = fixture.shown_on("desktop.bar.status", RIGHT);
    let band = fixture.shown_on("dock.bar.band.bottom", LEFT);
    assert!(
        matches!((&left[..], &band[..]), ([bar], [band]) if bar.intersects(*band) && bar.right() > LEFT.center().x),
        "the first display keeps its Status bar at the end of its foot: {left:?} {band:?}"
    );
    assert!(
        matches!(right[..], [bar] if bar.left() == RIGHT.left() && bar.height() > bar.width()),
        "the second display's Status bar runs down its left edge: {right:?}"
    );
}

#[test]
fn a_bar_moved_on_one_display_stays_put_on_the_others() {
    let mut fixture = Fixture::new();
    fixture.on_two_displays((LEFT, 1.0), (RIGHT, 1.0));
    fixture.display_bar_menu(1, STATUS_BAR, RIGHT, &["beside.ends", "beside.left.top"]);
    status_down_the_left_of_the_right_display_only(&fixture);
    fixture.test.desk_snapshot(
        "a_status_bar_moved_down_the_left_of_the_second_display_only",
        1,
    );

    let mut fixture = Fixture::resumed(fixture.saved());
    fixture.on_two_displays((LEFT, 1.0), (RIGHT, 1.0));
    status_down_the_left_of_the_right_display_only(&fixture);
}
