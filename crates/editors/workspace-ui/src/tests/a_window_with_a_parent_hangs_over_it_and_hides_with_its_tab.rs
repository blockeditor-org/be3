use super::*;

#[test]
fn a_window_with_a_parent_hangs_over_it_and_hides_with_its_tab() {
    let (mut fixture, _) = editor();

    fixture.host.set_watched::<WindowsWatch>(&vec![
        window(1, "Editor", None),
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
    let tab = (1u64 << 41) + 2;
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

    fixture.host.set_watched::<WindowsWatch>(&vec![
        window(1, "Editor", None),
        window(2, "Save changes?", Some(1)),
        window(3, "Terminal", None),
    ]);
    fixture.settle();
    let placed: Vec<u64> = placed_windows(&fixture).iter().map(|(id, _)| *id).collect();
    assert_eq!(
        placed,
        vec![3],
        "a window shown in its parent's place hides the dialog with the parent"
    );
}
