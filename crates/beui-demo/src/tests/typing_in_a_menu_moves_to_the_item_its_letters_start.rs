use super::heading_for_a_submenu_across_another_row_keeps_it_open::open_card_menu;
use super::*;
use std::time::Duration;

const FORGOTTEN: Duration = Duration::from_millis(1100);

fn typed(test: &mut DocumentTest, text: &str) {
    test.type_text(text);
    test.frame(Vec::new());
}

#[test]
fn typing_in_a_menu_moves_to_the_item_its_letters_start() {
    let mut test = alone(WIDE, Page::Menus);
    open_card_menu(&mut test);

    typed(&mut test, "s");
    assert!(focused(&test, "demo.menu.share"));
    typed(&mut test, "h");
    assert!(focused(&test, "demo.menu.share"), "\"sh\" still starts Share");
    typed(&mut test, "x");
    assert!(focused(&test, "demo.menu.share"), "\"shx\" starts nothing");

    test.pause(FORGOTTEN);
    typed(&mut test, "p");
    assert!(
        focused(&test, "demo.menu.share"),
        "Paste is disabled, so typing its letter skips it"
    );

    test.pause(FORGOTTEN);
    typed(&mut test, "e");
    assert!(focused(&test, "demo.menu.export"));
    typed(&mut test, "d");
    assert!(focused(&test, "demo.menu.export"), "\"ed\" starts nothing");
    test.pause(FORGOTTEN);
    typed(&mut test, "d");
    assert!(focused(&test, "demo.menu.delete"));
    test.pause(FORGOTTEN);
    typed(&mut test, "c");
    assert!(focused(&test, "demo.menu.copy"), "the search wraps around");

    test.pause(FORGOTTEN);
    typed(&mut test, "s");
    press_key(&mut test, beui::Key::ArrowRight);
    test.settle();
    assert!(focused(&test, "demo.menu.share.email"));
    typed(&mut test, "m");
    assert!(
        focused(&test, "demo.menu.share.message"),
        "a submenu has its own typeahead"
    );
    test.snapshot("typing_in_a_menu_moves_to_the_item_its_letters_start");
}
