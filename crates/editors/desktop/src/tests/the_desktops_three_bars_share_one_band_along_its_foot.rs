use super::*;

#[test]
fn the_desktops_three_bars_share_one_band_along_its_foot() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    let band = fixture.test.rect_of("dock.bar.band.bottom");
    assert!(
        band.left() < 0.5
            && (band.right() - 800.0).abs() < 0.5
            && (band.bottom() - 600.0).abs() < 0.5,
        "one band runs along the whole foot: {band:?}"
    );
    let apps = fixture.test.rect_of("desktop.bar.apps");
    let windows = fixture.test.rect_of("desktop.bar.windows");
    let status = fixture.test.rect_of("desktop.bar.status");
    for bar in [apps, windows, status] {
        assert!(
            band.contains_rect(bar),
            "each bar lies in the band: {bar:?} {band:?}"
        );
    }
    assert!(
        (apps.left() - band.left()).abs() < 0.5,
        "the Apps bar starts it: {apps:?}"
    );
    assert!(
        (status.right() - band.right()).abs() < 0.5,
        "the Status bar ends it: {status:?}"
    );
    assert!(
        (windows.center().x - band.center().x).abs() < 1.0
            && apps.right() < windows.left()
            && windows.right() < status.left(),
        "the Windows bar sits in the middle between them: {apps:?} {windows:?} {status:?}"
    );
    for part in ["desktop.apps", "desktop.windows.1", "desktop.windows.2"] {
        assert!(fixture.test.shown(part), "{part} is on the bars");
    }
    assert!(
        fixture.test.rect_of("desktop.status").right()
            < fixture.test.rect_of("desktop.clock").left(),
        "the clock ends the Status bar"
    );
    fixture.test.snapshot("the_desktop_bars_in_one_band");
}
