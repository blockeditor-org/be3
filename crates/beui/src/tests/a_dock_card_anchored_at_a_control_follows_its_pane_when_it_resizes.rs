use super::*;

#[test]
fn a_dock_card_anchored_at_a_control_follows_its_pane_when_it_resizes() {
    let _root = crate::reactive::RootScope::new();
    let dock = anchored_card_dock(true);
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    let anchor = open_anchored(&mut harness, (dock.anchor, dock.open), |panel| {
        Rect::from_min_size(
            pos2(panel.right() - 48.0, panel.top() + 16.0),
            vec2(32.0, 24.0),
        )
    });
    let output = harness.frame(Vec::new());
    let panel = harness.rect(harness.find("content.1"));
    let tip = beak(&output).expect("the card points at the control");
    assert!(
        (tip.x - anchor.center().x).abs() < 1.0,
        "{tip:?} for {anchor:?}"
    );

    harness.resize(Vec2::new(1300.0, 600.0));
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    let wider = harness.rect(harness.find("content.1"));
    assert!(
        wider.right() > panel.right() + 40.0,
        "the pane grows with the window: {wider:?} from {panel:?}"
    );
    let moved = beak(&output).expect("the card still points at the control");
    assert!(
        ((wider.right() - moved.x) - (panel.right() - tip.x)).abs() < 1.0
            && ((moved.y - wider.top()) - (tip.y - panel.top())).abs() < 1.0,
        "at the same place from the pane's nearer edges: {moved:?} in {wider:?}, was {tip:?} in {panel:?}"
    );
    let card = harness.rect(harness.find("dock.dialog.10"));
    assert!(
        card.top() >= moved.y && card.left() < moved.x && card.right() > moved.x,
        "with the card under it: {card:?}"
    );
}
