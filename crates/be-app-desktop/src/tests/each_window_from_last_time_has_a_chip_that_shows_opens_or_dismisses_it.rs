use be_sdk_beui::beui::PointerButton;

use super::*;

fn front_at(fixture: &Fixture, at: Pos2) -> Option<ChildContent> {
    fixture
        .test
        .children()
        .iter()
        .rev()
        .find(|placement| placement.clip.contains_half_open(at))
        .map(|placement| placement.content.clone())
}

#[test]
fn each_window_from_last_time_has_a_chip_that_shows_opens_or_dismisses_it() {
    let mut before = Fixture::new();
    before.settle();
    before.show_windows(&[foot(1), notes(6)]);
    let overlap = before
        .window_rect(1)
        .zip(before.window_rect(6))
        .map(|(foot, notes)| foot.intersect(notes))
        .expect("both windows are placed");
    assert!(overlap.is_positive(), "the windows overlap");
    let saved = before.saved();

    let mut resumed = Fixture::resumed(saved.clone());
    resumed.show_windows(&[host_window(9, true)]);
    let chip = |fixture: &Fixture, id: &str| {
        fixture
            .test
            .rect_of(&format!("desktop.windows.{id}"))
            .left()
    };
    assert!(
        chip(&resumed, "restore") < chip(&resumed, "9")
            && chip(&resumed, "9") < chip(&resumed, "placeholder.1")
            && chip(&resumed, "placeholder.1") < chip(&resumed, "placeholder.6"),
        "each window from last time has a chip after the running windows'"
    );
    assert!(
        resumed.says("Foot — Terminal 1"),
        "a chip names the window it stands for"
    );
    resumed.test.settle();
    resumed
        .test
        .snapshot("the_windows_bar_with_windows_from_last_time");

    resumed.test.clear_calls();
    resumed
        .test
        .click_with("desktop.windows.placeholder.1", PointerButton::Secondary);
    resumed.settle();
    assert!(
        !resumed.test.shown("desktop.windows.placeholder.1.visibility")
            && !resumed.test.shown("desktop.windows.placeholder.1.close"),
        "a window that is not running cannot be hidden or closed"
    );
    resumed
        .test
        .snapshot("the_menu_of_a_window_from_last_time");
    resumed.test.click("desktop.windows.placeholder.1.open");
    resumed.settle();
    assert_eq!(
        resumed
            .test
            .take_host_calls(be_sdk_test::requested::program),
        vec![ProgramAction::Launch {
            program: "foot.desktop".to_owned(),
            launch: Some(1),
        }],
        "its menu's Open starts the program"
    );

    resumed
        .test
        .click_with("desktop.windows.placeholder.6", PointerButton::Secondary);
    resumed.settle();
    resumed.test.click("desktop.windows.placeholder.6.dismiss");
    resumed.settle();
    assert!(
        !resumed.test.shown("desktop.windows.placeholder.6") && resumed.placeholder_rect(6).is_none(),
        "its menu's Dismiss forgets the window"
    );
    assert_eq!(kept_keys(&resumed), vec![1, 9]);

    let mut fronted = Fixture::resumed(saved);
    fronted.test.click("desktop.windows.placeholder.1");
    fronted.settle();
    fronted.show_windows(&[foot(1), notes(6)]);
    assert_eq!(
        front_at(&fronted, overlap.center()),
        Some(ChildContent::Window(HostWindowId(1))),
        "clicking a chip brings its placeholder in front"
    );
}
