use block_editor_beui::beui::Vec2;

use super::*;

#[test]
fn a_bar_menu_and_its_submenu_fit_the_work_area_of_a_short_window() {
    let mut fixture = Fixture::with_windows(&[1]);
    fixture.test.set_size(Vec2::new(800.0, 320.0));
    fixture.settle();
    fixture.bar_menu(STATUS_BAR, &["beside.ends"]);

    let band = fixture.test.rect_of("desktop.bar.windows");
    let row = fixture.test.rect_of("grip.dock.bar.beside.ends");
    let last = fixture.test.rect_of("grip.dock.bar.beside.right.bottom");
    assert!(
        row.top() >= 0.0 && row.bottom() <= band.top() + 0.5,
        "the grip menu scrolled its row into view above the band: {row:?} {band:?}"
    );
    assert!(
        last.top() >= 0.0 && last.bottom() <= band.top() + 0.5 && last.right() <= row.left() + 0.5,
        "the submenu opens to the left, above the band along the foot: {last:?} {row:?} {band:?}"
    );
    fixture
        .test
        .snapshot("a_bar_menu_and_its_submenu_on_a_short_window");

    fixture.test.click("grip.dock.bar.beside.right.bottom");
    fixture.settle();
    let side = fixture.test.rect_of("desktop.bar.status");
    assert!(
        side.left() > band.right(),
        "and choosing its last row docks the bar down the right: {side:?} {band:?}"
    );
}
