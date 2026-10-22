use super::*;

const SMALL: Vec2 = Vec2::new(720.0, 340.0);
const WINDOW_MARGIN: f32 = 12.0;

#[test]
fn a_card_keeps_inside_a_small_window() {
    let mut test = demo(SMALL);
    reveal(&mut test, "demo.dialog.card");

    test.click("demo.dialog.card");
    test.frame(Vec::new());
    let card = test.rect_of(&dialog_id(0));
    let window = beui::Rect::from_min_size(beui::Pos2::ZERO, SMALL).shrink(WINDOW_MARGIN);
    assert!(
        window.contains_rect(card),
        "the card keeps a margin inside a window too small for it: {card:?} in {window:?}"
    );
    let waiting = test.rect_of(&format!("dock.tab.{}.waiting", tab(Page::Docking)));
    assert!(
        card.top() > waiting.bottom(),
        "the card shrinks under its tab bar rather than hiding the waiting dot: {card:?} over {waiting:?}"
    );
    test.snapshot("docking_card_in_a_small_window");
}
