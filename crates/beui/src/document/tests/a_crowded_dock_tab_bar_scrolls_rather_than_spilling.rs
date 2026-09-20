use super::*;

#[test]
fn a_crowded_dock_tab_bar_scrolls_rather_than_spilling() {
    let (document, dock) = dock_of(12);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let area = harness.rect(dock);
    let last = dock_tab(harness.document(), dock, "Tab 12");
    assert!(
        harness.rect(last).left() > area.right(),
        "the tabs a pane has no room for sit past its edge"
    );

    harness.click(harness.center(dock_tab(harness.document(), dock, "Tab 1")));
    harness.key(Key::End, Modifiers::NONE);
    harness.frame(Vec::new());

    let last = dock_tab(harness.document(), dock, "Tab 12");
    assert!(
        harness.rect(last).right() <= area.right(),
        "walking to the last tab scrolls the bar until it is in view"
    );
    let first = dock_tab(harness.document(), dock, "Tab 1");
    assert!(
        harness.rect(first).right() < area.left(),
        "the tabs it scrolled past are the ones that leave"
    );
}
