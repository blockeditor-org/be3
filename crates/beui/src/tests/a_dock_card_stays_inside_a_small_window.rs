use super::*;

const SMALL_WINDOW: Vec2 = Vec2::new(760.0, 220.0);
const WINDOW_MARGIN: f32 = 12.0;

#[test]
fn a_dock_card_stays_inside_a_small_window() {
    let _root = crate::reactive::RootScope::new();
    let dock = side_card_dock(false);
    let mut harness = Harness::sized(dock.document, SMALL_WINDOW);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let panel = harness.rect(harness.find("content.3"));
    let card = harness.rect(harness.find("dock.dialog.10"));
    let window = Rect::from_min_size(Pos2::ZERO, SMALL_WINDOW).shrink(WINDOW_MARGIN);
    assert!(
        window.contains_rect(card),
        "the card keeps a margin inside the window: {card:?} in {window:?}"
    );
    assert!(
        card.center().x < panel.center().x - 1.0,
        "it moves off the centre of its pane rather than past the window's edge: {card:?} over {panel:?}"
    );
    assert_eq!(
        card.width(),
        CARD_SIZE.x,
        "it keeps its width while the window has room for it"
    );
    assert!(
        card.height() < CARD_SIZE.y,
        "and shrinks only where the window is too small: {card:?}"
    );
}
