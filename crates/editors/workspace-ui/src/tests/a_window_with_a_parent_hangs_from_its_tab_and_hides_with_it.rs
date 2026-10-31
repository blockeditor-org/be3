use super::*;

#[test]
fn a_window_with_a_parent_hangs_from_its_tab_and_hides_with_it() {
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
    let parent_tab = fixture
        .test
        .rect_of(&format!("dock.tab.{}", (1u64 << 41) + 1));
    assert!(
        dialog.top() > parent_tab.bottom()
            && dialog.left() <= parent_tab.center().x
            && parent_tab.center().x <= dialog.right()
            && dialog.top() < parent.center().y,
        "the dialog hangs from its parent's tab, over the parent: {dialog:?} below {parent_tab:?} over {parent:?}"
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
    drop(document);

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
