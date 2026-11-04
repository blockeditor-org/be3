use super::*;

#[test]
fn a_card_opened_from_a_button_flips_above_it_near_the_bottom() {
    let mut test = demo(WIDE);
    let (button, card) = open_pointing_card(&mut test);
    assert!(
        button.bottom() > WIDE.y - 200.0,
        "the page leaves the button near the bottom of the window: {button:?}"
    );
    assert!(
        card.bottom() < button.top() && card.bottom() > button.top() - 80.0,
        "with no room under the button the card opens just above it: {card:?} over {button:?}"
    );
    test.snapshot("docking_pointing_card_above");

    test.click(pos2(card.right() + 40.0, button.center().y));
    test.frame(Vec::new());
    assert!(
        !test.shows("demo.pointing.card"),
        "a press beside the card closes it"
    );
}
