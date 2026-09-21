use super::*;

#[test]
fn moving_a_dock_tab_to_another_pane_keeps_its_panel() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.click(harness.center(dock_tab(harness.document(), dock, "Tab 2")));
    harness.frame(Vec::new());
    let panel = harness.find("content.2");
    let before = harness.rect(panel);

    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    harness.drag(tab, pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y / 2.0));
    harness.frame(Vec::new());

    assert_eq!(
        harness.find("content.2"),
        panel,
        "the panel that moved with the tab is the one the tab already had"
    );
    let after = harness.rect(panel);
    assert!(
        after.left() > before.left() + 100.0,
        "the panel is laid out in the pane the tab was dropped in, \
         at {} rather than {}",
        after.left(),
        before.left()
    );
}
