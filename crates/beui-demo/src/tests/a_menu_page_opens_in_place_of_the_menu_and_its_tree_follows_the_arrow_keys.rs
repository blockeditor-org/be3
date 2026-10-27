use super::*;
use beui::Key;

#[test]
fn a_menu_page_opens_in_place_of_the_menu_and_its_tree_follows_the_arrow_keys() {
    let mut test = alone(WIDE, Page::Menus);
    reveal(&mut test, "demo.new");
    test.click("demo.new");
    test.click("demo.new.location");
    test.leave();
    assert!(
        test.shown("demo.new.location.back"),
        "the page leads with a row back to the menu"
    );
    assert!(
        !test.shown("demo.new.location"),
        "the page takes the menu's place"
    );
    assert!(
        focused(&test, "demo.new.folder.1"),
        "the keyboard starts on the checked folder"
    );
    test.snapshot("a_menu_page_shows_a_tree_with_the_chosen_folder_checked");

    press_key(&mut test, Key::ArrowDown);
    assert!(
        focused(&test, "demo.new.folder.2"),
        "Down moves to Projects"
    );
    press_key(&mut test, Key::ArrowRight);
    assert!(test.shown("demo.new.folder.3"), "Right opens Projects");
    press_key(&mut test, Key::ArrowRight);
    assert!(
        focused(&test, "demo.new.folder.3"),
        "Right on an open folder moves into it"
    );
    press_key(&mut test, Key::ArrowLeft);
    assert!(
        focused(&test, "demo.new.folder.2"),
        "Left on a leaf goes to its parent"
    );
    press_key(&mut test, Key::ArrowLeft);
    assert!(
        !test.shown("demo.new.folder.3"),
        "Left closes an open folder"
    );

    press_key(&mut test, Key::ArrowRight);
    press_key(&mut test, Key::ArrowDown);
    press_key(&mut test, Key::Enter);
    assert!(
        test.shown("demo.new.location") && focused(&test, "demo.new.location"),
        "choosing a folder goes back to the menu, on the row that opened the page"
    );
    test.click("demo.new.location");
    test.leave();
    assert!(
        focused(&test, "demo.new.folder.3"),
        "the folder chosen is the one checked now"
    );
    press_key(&mut test, Key::Escape);
    assert!(
        test.shown("demo.new.location"),
        "Escape goes back a page rather than closing the menu"
    );
    press_key(&mut test, Key::Escape);
    assert!(
        !test.shown("demo.new.location"),
        "Escape on the menu closes it"
    );
}
