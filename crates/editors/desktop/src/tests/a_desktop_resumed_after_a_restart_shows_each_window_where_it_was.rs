use super::*;

#[test]
fn a_desktop_resumed_after_a_restart_shows_each_window_where_it_was() {
    let mut before = Fixture::new();
    before.settle();
    before.show_windows(&[foot(1), notes(6)]);
    let places = [1, 6].map(|id| before.window_rect(id).expect("the window is placed"));

    let mut resumed = Fixture::resumed(before.saved());
    for (key, place) in [1, 6].into_iter().zip(places) {
        let shown = resumed
            .placeholder_rect(key)
            .unwrap_or_else(|| panic!("window {key} is shown as a placeholder"));
        assert_eq!(
            shown.min, place.min,
            "the placeholder for window {key} is where the window was"
        );
        assert_eq!(shown.size(), place.size(), "and the size it was");
    }
    assert!(resumed.says("Foot"), "a program it knows is named");
    assert!(resumed.says("Terminal 1"), "with the window's old title");
    assert!(
        resumed.says("org.example.Notes"),
        "a program it does not know goes by its app id"
    );
    assert!(resumed.test.shown("desktop.placeholders.open_all"));
    resumed.test.settle();
    resumed
        .test
        .snapshot("a_resumed_desktop_shows_its_windows_from_last_time");

    resumed.test.clear_calls();
    resumed.test.click("desktop.placeholders.open_all");
    resumed.settle();
    assert_eq!(
        resumed
            .test
            .take_host_calls(block_ui_test::requested::program),
        vec![
            ProgramAction::Launch {
                program: "foot.desktop".to_owned(),
                launch: Some(1),
            },
            ProgramAction::Exec {
                command: vec!["/usr/bin/notes".to_owned()],
                launch: Some(6),
            },
        ],
        "Open all starts every program: from its .desktop entry, else its command line"
    );

    resumed.test.click("desktop.placeholder.6.dismiss");
    resumed.settle();
    assert!(resumed.placeholder_rect(6).is_none());
    assert_eq!(
        kept_keys(&resumed),
        vec![1],
        "a dismissed placeholder is forgotten"
    );
}
