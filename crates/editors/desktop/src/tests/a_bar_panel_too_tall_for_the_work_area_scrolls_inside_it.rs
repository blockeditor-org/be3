use block_editor_beui::beui::Vec2;

use super::*;

const SHORT: Vec2 = Vec2::new(800.0, 320.0);

#[test]
fn a_bar_panel_too_tall_for_the_work_area_scrolls_inside_it() {
    let mut fixture = Fixture::with_windows(&[1]);
    fixture.hear(0.45, false, SPEAKERS);
    fixture.bar_menu(STATUS_BAR, &["beside.ends", "beside.right.bottom"]);
    fixture.test.set_size(SHORT);
    fixture.settle();

    fixture.test.click("desktop.status");
    fixture.settle();
    let foot = fixture.test.rect_of("desktop.bar.apps");
    let side = fixture.test.rect_of("desktop.bar.status");
    let quick = fixture.test.rect_of("desktop.quick");
    assert!(
        quick.top() >= 0.0 && quick.right() <= side.left(),
        "Quick settings starts inside the work area: {quick:?} {side:?}"
    );
    assert!(
        quick.bottom() > foot.top(),
        "and is taller than the work area, so it scrolls rather than covering the band along the foot: {quick:?} {foot:?}"
    );
    fixture
        .test
        .snapshot("a_bar_panel_scrolling_inside_the_work_area");
}
