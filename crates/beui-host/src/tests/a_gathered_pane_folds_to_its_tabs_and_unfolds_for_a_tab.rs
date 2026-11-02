use super::*;

#[test]
fn a_gathered_pane_folds_to_its_tabs_and_unfolds_for_a_tab() {
    let _root = crate::reactive::RootScope::new();
    let (document, layout) = staged_dock();
    let mut harness = Harness::sized(document, PHONE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let open = harness.rect(harness.find("dock.compact"));
    let stage_open = harness.rect(harness.find("content.1"));

    harness.click(harness.center(harness.find("dock.tab.2")));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let folded = harness.rect(harness.find("dock.compact"));
    let stage = harness.rect(harness.find("content.1"));
    assert!(
        folded.height() < open.height() / 2.0 && stage.height() > stage_open.height(),
        "tapping the shown tab folds the pane to its tab strip and gives the stage the room, {folded:?} {stage:?}"
    );
    assert!(
        harness.document().find_test_id("content.2").is_none()
            || !harness.rect(harness.find("content.2")).is_positive()
    );

    harness.click(harness.center(harness.find("dock.tab.3")));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let unfolded = harness.rect(harness.find("dock.compact"));
    assert!(
        (unfolded.height() - open.height()).abs() < 1.0,
        "tapping a tab unfolds the pane to its size, {unfolded:?}"
    );
    assert!(unfolded.contains_rect(harness.rect(harness.find("content.3"))));

    harness.click(harness.center(harness.find("dock.tab.3")));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    layout.show(&4);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let shown = harness.rect(harness.find("dock.compact"));
    assert!(
        (shown.height() - open.height()).abs() < 1.0,
        "a tab the code shows unfolds the pane, {shown:?}"
    );
    assert!(shown.contains_rect(harness.rect(harness.find("content.4"))));
}
