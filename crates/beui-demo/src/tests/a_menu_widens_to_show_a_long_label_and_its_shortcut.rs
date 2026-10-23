use super::*;

const OLD_WIDTH: f32 = 256.0;

#[test]
fn a_menu_widens_to_show_a_long_label_and_its_shortcut() {
    let mut test = alone(WIDE, Page::Menus);
    reveal(&mut test, "demo.actions");
    let card = test.rect_of("demo.actions");
    test.click_with(card.center(), beui::PointerButton::Secondary);
    test.settle();
    let long = test.rect_of("demo.actions.demo.menu.link");
    let short = test.rect_of("demo.actions.demo.menu.copy");
    assert_eq!(
        long.width(),
        short.width(),
        "every row of a menu is as wide as its widest"
    );
    assert!(
        long.width() > OLD_WIDTH,
        "the menu grows past a fixed width to hold its long row: {long:?}"
    );
    test.snapshot("a_menu_widens_to_show_a_long_label_and_its_shortcut");
}
