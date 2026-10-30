use super::*;

#[test]
fn floating_bars_float_over_the_windows_and_toasts_follow_the_status_bar() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture.bar_menu(WINDOWS_BAR, &["over.bottom"]);
    fixture.bar_menu(STATUS_BAR, &["over.ends", "over.top.right"]);
    let band = fixture.test.rect_of("dock.bar.band.bottom");
    let apps = fixture.test.rect_of("desktop.bar.apps");
    let windows = fixture.test.rect_of("desktop.bar.windows");
    let status = fixture.test.rect_of("desktop.bar.status");
    assert!(
        band.contains_rect(apps) && !band.contains_rect(windows),
        "only the Apps bar is left in the band: {band:?} {windows:?}"
    );
    assert!(
        windows.bottom() < band.top() && (windows.center().x - 400.0).abs() < 1.0,
        "the Windows bar floats over the windows just above the band: {windows:?}"
    );
    assert!(
        status.top() < 50.0 && status.right() > 750.0,
        "the Status bar floats at the top right: {status:?}"
    );
    fixture.test.snapshot("the_desktop_bars_floating");

    fixture.notify(&[notification(1, "Mail", "Mia Chen", "Lunch at noon?")]);
    let card = fixture.test.rect_of(&toast(1));
    assert!(
        card.top() < 100.0 && card.right() > 700.0,
        "toasts gather at the top right, by the Status bar: {card:?}"
    );
}
