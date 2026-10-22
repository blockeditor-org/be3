use super::*;

#[test]
fn a_dock_card_anchored_at_a_control_flips_above_it_near_the_bottom() {
    let _root = crate::reactive::RootScope::new();
    let dock = anchored_card_dock(false);
    let (pressed, attended, open) = (dock.pressed, dock.attended, dock.open);
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    let anchor = open_anchored(&mut harness, (dock.anchor, open), |panel| {
        Rect::from_min_size(
            pos2(panel.center().x - 16.0, panel.bottom() - 48.0),
            vec2(32.0, 24.0),
        )
    });
    let output = harness.frame(Vec::new());

    let panel = harness.rect(harness.find("content.1"));
    let card = harness.rect(harness.find("dock.dialog.10"));
    assert_eq!(card.size(), CARD_SIZE, "the card keeps its size");
    assert!(
        card.bottom() < anchor.top() && card.bottom() > anchor.top() - 24.0,
        "with no room under the control the card opens just above it: {card:?} over {anchor:?}"
    );
    let tip = beak(&output).expect("the card points at the control");
    assert!(
        (tip.x - anchor.center().x).abs() < 1.0 && tip.y >= card.bottom() && tip.y < anchor.top(),
        "from its bottom edge: {tip:?} for {anchor:?} under {card:?}"
    );

    harness.click(pos2(panel.center().x, panel.top() + 12.0));
    harness.frame(Vec::new());
    assert!(
        open.get_untracked(),
        "a card that did not ask to close on a press stays open"
    );
    assert_eq!(
        attended.get_untracked(),
        vec![CARD],
        "and calls attention instead"
    );
    assert!(
        pressed.with_untracked(Vec::is_empty),
        "while its tab hears nothing"
    );
    assert!(!dims(&output, panel), "and is never dimmed");
}
