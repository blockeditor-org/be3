use super::*;

#[test]
fn bars_docked_down_the_left_lay_their_parts_out_down_it() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture.hear(0.45, false, SPEAKERS);
    fixture.bar_menu(APPS_BAR, &["beside.ends", "beside.left.top"]);
    fixture.bar_menu(WINDOWS_BAR, &["beside.left"]);
    fixture.bar_menu(STATUS_BAR, &["beside.ends", "beside.left.bottom"]);
    assert!(
        !fixture.test.shown("dock.bar.band.bottom"),
        "no bar is left along the foot"
    );
    let band = fixture.test.rect_of("dock.bar.band.left");
    assert!(
        band.left() < 0.5 && band.top() < 0.5 && (band.bottom() - 600.0).abs() < 0.5,
        "one band runs down the whole left: {band:?}"
    );
    let apps = fixture.test.rect_of("desktop.bar.apps");
    let windows = fixture.test.rect_of("desktop.bar.windows");
    let status = fixture.test.rect_of("desktop.bar.status");
    assert!(
        (apps.top() - band.top()).abs() < 0.5
            && apps.bottom() < windows.top()
            && windows.bottom() < status.top()
            && (status.bottom() - band.bottom()).abs() < 0.5,
        "Apps at the top, Windows in the middle, Status at the foot: {apps:?} {windows:?} {status:?}"
    );
    let chip = fixture.test.rect_of("desktop.windows.1");
    assert!(
        chip.width() < band.width() && (chip.width() - chip.height()).abs() < 0.5,
        "a chip down the side shows only its icon: {chip:?}"
    );
    let clock = fixture.test.rect_of("desktop.clock");
    assert!(
        clock.height() > clock.width(),
        "the clock stacks its hours over its minutes: {clock:?}"
    );
    let first = fixture.test.rect_of("desktop.windows.1");
    let second = fixture.test.rect_of("desktop.windows.2");
    assert!(first.bottom() <= second.top(), "the chips run down the bar");
    fixture.test.snapshot("the_desktop_bars_down_the_left");

    fixture.test.click("desktop.status");
    fixture.settle();
    let quick = fixture.test.rect_of("desktop.quick");
    assert!(
        quick.left() >= band.right() - 0.5,
        "Quick settings opens away from the edge, toward the windows: {quick:?} {band:?}"
    );
    fixture
        .test
        .snapshot("quick_settings_beside_a_bar_down_the_left");
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();

    fixture.notify(&[notification(1, "Mail", "Mia Chen", "Lunch at noon?")]);
    let card = fixture.test.rect_of(&toast(1));
    assert!(
        card.left() >= band.right() && card.left() < 400.0 && card.bottom() > 500.0,
        "toasts gather in the corner nearest the Status bar: {card:?}"
    );
}
