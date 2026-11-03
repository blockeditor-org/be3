use be_sdk_beui::beui::PointerButton;

use super::*;

#[test]
fn a_window_with_a_dialog_hides_and_comes_back_with_it() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let window = |shown: bool| HostWindow { shown, ..foot(1) };
    let dialog = HostWindow {
        parent: Some(HostWindowId(1)),
        title: "Preferences".to_owned(),
        modal: true,
        focused: true,
        ..foot(2)
    };
    let other = |focused: bool| HostWindow { focused, ..foot(3) };
    let header = format!("dock.dialog.{}.header", window_tab(2));
    fixture.show_windows(&[window(true), other(false), dialog.clone()]);
    assert!(fixture.test.shown(&header), "the window's dialog is shown");

    fixture
        .test
        .click_with("desktop.windows.1", PointerButton::Secondary);
    fixture.settle();
    fixture.test.click("desktop.windows.1.visibility");
    fixture.settle();
    fixture.show_windows(&[
        window(false),
        other(true),
        HostWindow {
            focused: false,
            ..dialog.clone()
        },
    ]);
    assert!(fixture.window_rect(1).is_none(), "the window is hidden");
    assert!(!fixture.test.shown(&header), "and its dialog goes with it");

    fixture.show_windows(&[window(true), other(false), dialog.clone()]);
    assert!(
        fixture.test.shown(&header),
        "showing the window again brings its dialog back"
    );

    fixture.show_windows(&[other(true), dialog]);
    assert!(
        !fixture.test.shown(&header),
        "a closed window takes its dialog with it"
    );
}
