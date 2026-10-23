use super::*;

#[test]
fn a_dock_card_anchored_at_a_control_points_at_it_without_dimming_its_tab() {
    let _root = crate::reactive::RootScope::new();
    let dock = anchored_card_dock(true);
    let (pressed, attended, open) = (dock.pressed, dock.attended, dock.open);
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    let anchor = open_anchored(&mut harness, (dock.anchor, open), |panel| {
        Rect::from_min_size(panel.min + vec2(24.0, 16.0), vec2(32.0, 24.0))
    });
    let output = harness.frame(Vec::new());

    let panel = harness.rect(harness.find("content.1"));
    let card = harness.rect(harness.find("dock.dialog.10"));
    assert_eq!(card.size(), CARD_SIZE, "the card keeps its size");
    assert!(
        card.top() > anchor.bottom() && card.top() < anchor.bottom() + 24.0,
        "the card opens just under the control that asked: {card:?} under {anchor:?}"
    );
    assert!(
        card.left() < panel.left(),
        "and reaches past its pane over the one beside it: {card:?} over {panel:?}"
    );
    let tip = beak(&output).expect("the card points at the control");
    assert!(
        (tip.x - anchor.center().x).abs() < 1.0 && tip.y > anchor.bottom() && tip.y <= card.top(),
        "from its top edge, at the middle of the control: {tip:?} for {anchor:?} over {card:?}"
    );
    assert!(!dims(&output, panel), "its tab is not dimmed");
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.1.waiting")
            .is_some(),
        "its tab still waits on it"
    );
    assert_eq!(
        harness.document().focused_node(),
        Some(harness.find("card.field")),
        "and it has the keyboard"
    );

    let side = harness.rect(harness.find("content.3"));
    harness.click(pos2(side.center().x, side.bottom() - 12.0));
    harness.frame(Vec::new());
    assert_eq!(
        pressed.get_untracked(),
        vec![3],
        "the pane beside it takes a press"
    );
    assert!(open.get_untracked(), "which leaves the card open");

    harness.click(pos2(panel.center().x, panel.bottom() - 12.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        !open.get_untracked(),
        "a press on its own tab closes the card"
    );
    assert_eq!(pressed.get_untracked(), vec![3], "without reaching the tab");
    assert!(
        attended.with_untracked(Vec::is_empty),
        "or calling attention"
    );
}
