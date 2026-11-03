use be_sdk_beui::beui::PointerButton;

use super::*;

fn hide(fixture: &mut Fixture, window: u64) {
    fixture.test.click_with(
        &format!("desktop.windows.{window}"),
        PointerButton::Secondary,
    );
    fixture.settle();
    fixture
        .test
        .click(&format!("desktop.windows.{window}.visibility"));
    fixture.settle();
}

#[test]
fn a_hidden_window_is_still_heard_and_its_programs_raise_brings_it_back() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    hide(&mut fixture, 1);
    assert!(fixture.window_rect(1).is_none(), "the window is hidden");

    fixture.show_windows(&[
        TestWindow {
            title: "notes.txt".to_owned(),
            ..host_window(1, false)
        },
        host_window(2, true),
    ]);
    assert_eq!(
        accessible_label_around(&fixture, "desktop.windows.1").as_deref(),
        Some("Kate \u{2014} notes.txt"),
        "a hidden window's new title reaches its chip"
    );
    assert!(
        fixture.window_rect(1).is_none(),
        "hearing it does not show it"
    );

    fixture.raise(1);
    assert!(
        fixture.window_rect(1).is_some(),
        "a hidden window its program raises comes back"
    );
}
