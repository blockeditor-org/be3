use super::*;

#[test]
fn super_brings_out_an_auto_hidden_apps_bar_with_its_panel() {
    let mut fixture = Fixture::with_windows(&[1]);
    fixture.auto_hidden();
    assert!(!fixture.test.shown("desktop.apps"));

    assert!(fixture.test.host_tap(Key::Logo));
    fixture.wait(500);
    let button = fixture.test.rect_of("desktop.apps");
    let panel = fixture.test.rect_of("desktop.apps.panel");
    assert!(
        (button.bottom() - 600.0).abs() < 8.0 && panel.bottom() <= button.top() + 1.0,
        "the band comes out and the panel opens from its button: {button:?} {panel:?}"
    );
    fixture.wait(1000);
    assert!(
        fixture.test.shown("desktop.apps.panel") && fixture.test.shown("desktop.apps"),
        "the band stays out while its panel is open"
    );

    assert!(fixture.test.host_tap(Key::Logo));
    fixture.wait(1000);
    assert!(
        !fixture.test.shown("desktop.apps.panel") && !fixture.test.shown("desktop.apps"),
        "once the panel closes, the band goes with the pointer away"
    );
}
