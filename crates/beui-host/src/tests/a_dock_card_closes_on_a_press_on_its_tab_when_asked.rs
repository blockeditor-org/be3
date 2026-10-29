use super::*;

#[test]
fn a_dock_card_closes_on_a_press_on_its_tab_when_asked() {
    let _root = crate::reactive::RootScope::new();
    let dock = card_dock(true);
    let (pressed, attended, open) = (dock.pressed, dock.attended, dock.open);
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let panel = harness.rect(harness.find("content.1"));
    let card = harness.rect(harness.find("dock.dialog.10"));
    let side = harness.rect(harness.find("content.3"));
    let left = harness.rect(harness.find("content.4"));

    harness.click(pos2(side.center().x, side.bottom() - 12.0));
    harness.frame(Vec::new());
    harness.click(pos2(left.center().x, left.bottom() - 12.0));
    harness.frame(Vec::new());
    assert_eq!(
        pressed.get_untracked(),
        vec![3, 4],
        "the panes beside the card take their presses"
    );
    assert!(
        open.get_untracked() && harness.document().find_test_id("dock.dialog.10").is_some(),
        "without closing it"
    );

    harness.click(harness.center(harness.find("card.field")));
    harness.frame(Vec::new());
    assert!(
        open.get_untracked(),
        "a press inside the card keeps it open"
    );

    harness.click(pos2(panel.center().x, card.bottom() + 24.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        !open.get_untracked(),
        "a press on the dimmed tab closes the card"
    );
    assert!(
        harness.document().find_test_id("dock.dialog.10").is_none(),
        "and it goes"
    );
    assert_eq!(
        pressed.get_untracked(),
        vec![3, 4],
        "the press that closed it never reaches the tab"
    );
    assert!(
        attended.with_untracked(Vec::is_empty),
        "and calls no attention"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.1.waiting")
            .is_none(),
        "the tab stops waiting"
    );
}
