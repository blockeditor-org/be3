use super::*;

#[test]
fn a_dock_card_reaches_past_its_pane_and_holds_only_its_tab() {
    let _root = crate::reactive::RootScope::new();
    let dock = card_dock(false);
    let (layout, pressed, built, attended, open) = (
        dock.layout,
        dock.pressed,
        dock.built,
        dock.attended,
        dock.open,
    );
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let panel = harness.rect(harness.find("content.1"));
    let card = harness.rect(harness.find("dock.dialog.10"));
    assert_eq!(
        card.size(),
        CARD_SIZE,
        "the card keeps the size it asked for although its pane is narrower: {card:?} over {panel:?}"
    );
    assert!(
        (card.center().x - panel.center().x).abs() < 1.0,
        "the card is centred on its tab's panel: {card:?} over {panel:?}"
    );
    assert!(
        card.top() > panel.top() && card.top() < panel.top() + 24.0,
        "the card floats a small gap under the tab bar: {card:?} under {panel:?}"
    );
    assert!(
        card.left() < panel.left() && card.right() > panel.right(),
        "and reaches past its pane on both sides: {card:?} over {panel:?}"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.1.waiting")
            .is_some(),
        "its tab shows that it waits on the card"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.2.waiting")
            .is_none(),
        "no other tab does"
    );
    let covers = harness.document().overlay_rects(crate::WindowId::MAIN);
    assert!(
        covers.iter().any(|cover| cover.contains_rect(panel))
            && covers.iter().any(|cover| cover.contains_rect(card))
            && !covers
                .iter()
                .any(|cover| cover.contains_rect(panel.union(card))),
        "the dimmed panel and the card each cover what lies under them, and nothing between: {covers:?}"
    );
    let field = harness.find("card.field");
    assert_eq!(
        harness.document().focused_node(),
        Some(field),
        "the card takes the focus"
    );

    harness.click(harness.center(field));
    harness.frame(Vec::new());
    assert!(
        attended.with_untracked(Vec::is_empty),
        "a press inside the card calls no attention"
    );

    let below = pos2(panel.center().x, panel.bottom() - 12.0);
    harness.click(below);
    harness.frame(Vec::new());
    assert!(
        pressed.with_untracked(Vec::is_empty),
        "the dimmed tab hears no press"
    );
    assert_eq!(
        attended.get_untracked(),
        vec![CARD],
        "a press on it calls the card's attention"
    );
    assert!(
        open.get_untracked(),
        "without closing a card that did not ask to"
    );

    let side = harness.rect(harness.find("content.3"));
    assert!(
        side.contains(pos2(card.right() - 10.0, card.center().y)),
        "the card reaches over the pane beside it: {card:?} over {side:?}"
    );
    harness.click(pos2(card.right() - 10.0, card.center().y));
    harness.frame(Vec::new());
    assert!(
        pressed.with_untracked(Vec::is_empty),
        "where the card covers the other pane, the card takes the press"
    );

    let side_node = harness.find("content.3");
    harness.click(pos2(side.center().x, side.bottom() - 12.0));
    harness.frame(Vec::new());
    assert_eq!(
        pressed.get_untracked(),
        vec![3],
        "the other pane still takes presses beside the card"
    );
    assert_eq!(
        harness.document().focused_node(),
        Some(side_node),
        "and the focus"
    );
    assert_eq!(
        attended.get_untracked(),
        vec![CARD],
        "without calling the card's attention"
    );
    assert!(
        harness.document().find_test_id("dock.dialog.10").is_some(),
        "or closing it"
    );

    layout.show(&2);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("dock.dialog.10").is_none(),
        "the card hides while another tab is shown in its place"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.1.waiting")
            .is_some(),
        "its tab still shows that it waits"
    );
    let other = harness.find("content.2");
    harness.click(harness.center(other));
    assert_eq!(
        pressed.get_untracked(),
        vec![3, 2],
        "the tab shown in its place takes presses"
    );

    layout.show(&1);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(harness.find("dock.dialog.10")),
        card,
        "the card comes back where it was"
    );
    assert_eq!(harness.find("card.field"), field, "with the content it had");
    assert_eq!(built.get_untracked(), 1, "which was never built again");
    assert_eq!(
        harness.document().focused_node(),
        Some(field),
        "and the focus goes back into it"
    );

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(!open.get_untracked(), "Escape asks the card to close");
    assert!(
        harness.document().find_test_id("dock.dialog.10").is_none(),
        "and it closes"
    );
    assert!(
        harness
            .document()
            .find_test_id("dock.tab.1.waiting")
            .is_none(),
        "taking the dot with it"
    );
    harness.click(below);
    assert_eq!(
        pressed.get_untracked(),
        vec![3, 2, 1],
        "the tab takes presses again"
    );
}
