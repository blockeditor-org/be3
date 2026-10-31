use super::*;

#[test]
fn a_hidden_bar_stays_hidden_and_the_apps_panel_shows_it_again() {
    let mut fixture = Fixture::with_windows(&[1]);
    assert!(!fixture.test.shown("desktop.apps.bars") && fixture.test.shown("desktop.apps"));
    fixture.bar_menu(APPS_BAR, &["hide"]);
    assert!(
        !fixture.test.shown("desktop.bar.apps") && !fixture.test.shown("desktop.apps"),
        "the hidden bar and its button are gone"
    );
    assert!(fixture.test.shown("desktop.bar.windows") && fixture.test.shown("desktop.bar.status"));

    let mut fixture = Fixture::resumed(fixture.saved());
    assert!(
        !fixture.test.shown("desktop.apps"),
        "the desktop keeps the bar hidden when it starts again"
    );
    assert!(fixture.test.app_tap(Key::Logo));
    fixture.settle();
    let band = fixture.test.rect_of("dock.bar.band.bottom");
    let panel = fixture.test.rect_of("desktop.apps.panel");
    assert!(
        (panel.center().x - 400.0).abs() < 1.0 && (panel.center().y - band.top() / 2.0).abs() < 1.0,
        "Super opens Apps centred over the windows' area: {panel:?} {band:?}"
    );
    assert!(fixture.test.shown("desktop.apps.bars"));
    fixture.test.snapshot("the_apps_panel_with_a_hidden_bar");

    fixture.test.click("desktop.apps.bars");
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.bar.apps") && fixture.test.shown("desktop.apps"),
        "the bar is back"
    );
    assert!(!fixture.test.shown("desktop.apps.panel"));
    fixture.test.click("desktop.apps");
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.apps.bars"),
        "with every bar shown the panel has none to show"
    );
}
