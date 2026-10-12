use super::*;

#[test]
fn a_window_with_a_parent_hangs_over_it() {
    let mut fixture = Fixture::new();
    fixture.settle();

    fixture.test.set_host_value::<HostWindows>(&vec![
        large_window(1, "Editor"),
        window(2, "Save changes?", Some(1)),
    ]);
    fixture.settle();

    let placed = placed_windows(&fixture);
    let rect = |id: u64| {
        placed
            .iter()
            .find(|(placed, _)| *placed == id)
            .map(|(_, rect)| *rect)
    };
    let parent = rect(1).expect("the parent is placed");
    let dialog = rect(2).expect("the dialog is placed");
    assert_eq!(
        (dialog.width(), dialog.height()),
        (320.0, 200.0),
        "the dialog is the size it drew rather than filling a pane"
    );
    assert!(
        (dialog.center().x - parent.center().x).abs() < 1.0 && parent.contains_rect(dialog),
        "the dialog hangs in the middle of its parent: {dialog:?} over {parent:?}"
    );
    let tab = window_tab(2);
    let document = fixture.test.document();
    assert!(
        document
            .find_test_id(&format!("dock.dialog.{tab}"))
            .is_some()
            && document
                .find_test_id(&format!("dock.tab.{tab}.close"))
                .is_none(),
        "the dialog has no tab of its own"
    );
}
