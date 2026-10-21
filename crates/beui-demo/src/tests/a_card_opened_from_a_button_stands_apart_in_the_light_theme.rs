use super::*;

const TALL: Vec2 = Vec2::new(1100.0, 1700.0);

#[test]
fn a_card_opened_from_a_button_stands_apart_in_the_light_theme() {
    let mut test = demo_in(TALL, beui::ThemeKind::Light);
    let (button, card) = open_pointing_card(&mut test);
    assert!(card.top() > button.bottom(), "{card:?} under {button:?}");
    test.snapshot("docking_pointing_card_light");
}
