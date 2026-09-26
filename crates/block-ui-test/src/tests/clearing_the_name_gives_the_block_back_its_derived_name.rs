use super::*;

use beui::{Key, Modifiers};

#[test]
fn clearing_the_name_gives_the_block_back_its_derived_name() {
    let (mut test, store, block) = named_editor();
    let mut info = store.block(block).unwrap();
    info.name = Some("Plans".to_owned());
    info.named_by_hand = true;
    store.add_block(info);
    test.run();
    assert_eq!(shown_name(&test), "Plans");

    test.click("editor.name");
    test.key_press_modifiers(Modifiers::CTRL, Key::A);
    test.key_press(Key::Backspace);
    test.key_press(Key::Enter);
    test.run();
    test.run();

    assert_eq!(name(&store, block), None, "the manual name is gone");
    assert_eq!(shown_name(&test), "");
}
