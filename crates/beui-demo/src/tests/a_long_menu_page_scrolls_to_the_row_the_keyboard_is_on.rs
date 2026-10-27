use super::*;
use beui::Key;

#[test]
fn a_long_menu_page_scrolls_to_the_row_the_keyboard_is_on() {
    let mut test = alone(WIDE, Page::Menus);
    reveal(&mut test, "demo.new");
    test.click("demo.new");
    test.click("demo.new.more");
    let back = test.rect_of("demo.new.more.back");
    press_key(&mut test, Key::End);
    let root = test.document().root().expect("the demo has a root");
    assert!(
        showing(test.document(), root, "Whiteboard") > 0,
        "End scrolls the last row into the menu"
    );
    assert!(
        !test.shown("demo.new.more.back") || test.rect_of("demo.new.more.back").top() < back.top(),
        "the rows above scrolled up out of the way"
    );
    test.snapshot("a_long_menu_page_scrolls_to_its_last_row");
    press_key(&mut test, Key::ArrowLeft);
    assert!(test.shown("demo.new.more"), "Left goes back to the menu");
}
