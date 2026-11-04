use super::*;

#[test]
fn a_window_launched_from_a_placeholder_takes_its_place() {
    let mut before = Fixture::new();
    before.settle();
    before.show_windows(&[foot(1), foot(2)]);
    let [older, newer] = [1, 2].map(|id| before.tab_of(id));
    let mut resumed = Fixture::resumed(before.saved());
    let place = resumed
        .placeholder_rect(newer)
        .expect("the newer terminal is a placeholder");
    resumed.test.clear_calls();

    resumed
        .test
        .click(&format!("workspace.placeholder.{newer}.open"));
    resumed.settle();
    assert_eq!(
        resumed
            .test
            .take_host_calls(be_sdk_test::requested::program),
        vec![ProgramAction::Launch {
            program: "foot.desktop".to_owned(),
            launch: Some(newer),
        }],
        "Open starts the program, naming the placeholder it was started from"
    );

    let launched = TestWindow {
        origin: WindowOrigin {
            launch: Some(newer),
            ..foot(50).origin
        },
        ..foot(50)
    };
    resumed.show_windows(std::slice::from_ref(&launched));
    assert!(
        resumed.placeholder_rect(newer).is_none(),
        "the new window replaces the placeholder it was started from"
    );
    assert!(
        resumed.placeholder_rect(older).is_some(),
        "even though an older one of the same program is waiting"
    );
    let shown = resumed.window_rect(50).expect("the new window is placed");
    assert_eq!(shown.min, place.min, "in the placeholder's place");

    let mut again = Fixture::holding(resumed.saved());
    again.show_windows(std::slice::from_ref(&launched));
    assert_eq!(
        again.window_rect(50).map(|rect| rect.min),
        Some(place.min),
        "a shell started again over the same windows keeps it in the place it took"
    );
    assert!(again.placeholder_rect(newer).is_none());
    assert!(again.placeholder_rect(older).is_some());

    resumed.show_windows(&[]);
    assert!(
        resumed.placeholder_rect(newer).is_none(),
        "closing it closes it for good"
    );
    assert_eq!(kept_keys(&resumed), vec![older]);
}
