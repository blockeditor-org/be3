use super::*;

const TALL: Vec2 = Vec2::new(1100.0, 1700.0);

#[test]
fn a_card_opened_from_a_button_opens_under_it_where_there_is_room() {
    let mut test = demo(TALL);
    let (button, card) = open_pointing_card(&mut test);
    assert!(
        card.top() > button.bottom() && card.top() < button.bottom() + 100.0,
        "the card opens just under the button: {card:?} under {button:?}"
    );
    assert!(
        card.left() < button.center().x && card.right() > button.center().x,
        "around the middle of it: {card:?} for {button:?}"
    );
    test.snapshot("docking_pointing_card");
}
