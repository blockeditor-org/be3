use block_editor_beui::beui::styled::popover::PANEL_PADDING;

use super::*;

#[test]
fn a_bar_panel_opens_just_past_its_bar_on_any_edge() {
    let mut fixture = Fixture::with_windows(&[1]);
    let past = |gap: f32| gap >= PANEL_PADDING.px() - 0.5 && gap <= PANEL_PADDING.px() + 1.5;

    fixture.test.click("desktop.status");
    fixture.settle();
    let bar = fixture.test.rect_of("desktop.bar.status");
    let quick = fixture.test.rect_of("desktop.quick");
    assert!(
        past(bar.top() - quick.bottom()),
        "Quick settings opens just above the bar along the foot, not over its padding: {quick:?} {bar:?}"
    );
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();

    fixture.bar_menu(STATUS_BAR, &["beside.ends", "beside.right.bottom"]);
    fixture.test.click("desktop.status");
    fixture.settle();
    let bar = fixture.test.rect_of("desktop.bar.status");
    let quick = fixture.test.rect_of("desktop.quick");
    assert!(
        past(bar.left() - quick.right()),
        "and just left of a bar down the right: {quick:?} {bar:?}"
    );
    fixture
        .test
        .snapshot("quick_settings_just_past_a_bar_down_the_right");
}
