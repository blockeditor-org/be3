use super::*;

const NARROW: Vec2 = Vec2::new(900.0, 1700.0);
const WINDOW_MARGIN: f32 = 12.0;

#[test]
fn a_card_opened_from_a_button_shifts_inside_a_narrow_window() {
    let mut test = demo(NARROW);
    let (button, card) = open_pointing_card(&mut test);
    let window = beui::Rect::from_min_size(beui::Pos2::ZERO, NARROW).shrink(WINDOW_MARGIN);
    assert!(
        window.contains_rect(card),
        "the card keeps a margin inside the window: {card:?} in {window:?}"
    );
    assert!(
        card.center().x < button.center().x - 40.0,
        "moving along rather than centring on the button: {card:?} for {button:?}"
    );
    assert!(
        card.top() > button.bottom() && card.right() > button.center().x,
        "still under it: {card:?} for {button:?}"
    );
    test.snapshot("docking_pointing_card_shifted");
}
