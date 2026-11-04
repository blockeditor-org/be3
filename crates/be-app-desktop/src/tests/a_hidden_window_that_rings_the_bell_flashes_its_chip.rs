use be_sdk_beui::beui::PointerButton;

use super::*;

#[test]
fn a_hidden_window_that_rings_the_bell_flashes_its_chip() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture
        .test
        .click_with("desktop.windows.2", PointerButton::Secondary);
    fixture.settle();
    fixture.test.click("desktop.windows.2.visibility");
    fixture.settle();
    assert!(fixture.window_rect(2).is_none(), "the window is hidden");

    fixture.show_windows(&[
        host_window(1, true),
        TestWindow {
            rung: 1,
            ..host_window(2, false)
        },
    ]);
    assert!(
        fixture.window_rect(2).is_none(),
        "its bell does not show it"
    );
    fixture
        .test
        .snapshot("a_hidden_window_that_rings_the_bell_flashes_its_chip");
}
