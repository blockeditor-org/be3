use super::*;

use beui::{Key, Modifiers};

#[test]
fn clearing_the_name_gives_the_block_back_its_derived_name() {
    let (mut test, client, block) = named_editor();
    client.get_block::<FileTree>(block).set_name("Plans");
    test.run();
    assert_eq!(shown_name(&test), "Plans");

    test.click("editor.name");
    test.key_press_modifiers(Modifiers::CTRL, Key::A);
    test.key_press(Key::Backspace);
    test.key_press(Key::Enter);
    test.run();
    test.run();

    assert_eq!(name(&client, block), None, "the manual name is gone");
    assert_eq!(shown_name(&test), "");
}
