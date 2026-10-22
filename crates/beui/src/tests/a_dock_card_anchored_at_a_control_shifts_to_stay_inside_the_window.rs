use super::*;

const NARROW: Vec2 = Vec2::new(760.0, 600.0);

#[test]
fn a_dock_card_anchored_at_a_control_shifts_to_stay_inside_the_window() {
    let _root = crate::reactive::RootScope::new();
    let dock = anchored_side_card_dock(true);
    let mut harness = Harness::sized(dock.document, NARROW);
    let anchor = open_anchored_in(
        &mut harness,
        (dock.anchor, dock.open),
        "content.3",
        |panel| {
            Rect::from_min_size(
                pos2(panel.right() - 64.0, panel.top() + 16.0),
                vec2(32.0, 24.0),
            )
        },
    );
    let output = harness.frame(Vec::new());

    let card = harness.rect(harness.find("dock.dialog.10"));
    assert_eq!(card.size(), CARD_SIZE, "the card keeps its size");
    assert!(
        card.right() < NARROW.x - 8.0,
        "it stays a margin inside the window: {card:?}"
    );
    assert!(
        card.center().x < anchor.center().x - 8.0,
        "by moving along rather than centring on the control: {card:?} for {anchor:?}"
    );
    assert!(card.top() > anchor.bottom(), "still under it: {card:?}");
    let tip = beak(&output).expect("the card points at the control");
    assert!(
        (tip.x - anchor.center().x).abs() < 1.0,
        "and its pointer still reaches the control: {tip:?} for {anchor:?}"
    );
}
