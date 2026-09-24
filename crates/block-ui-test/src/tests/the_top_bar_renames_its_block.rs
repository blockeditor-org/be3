use super::*;

use beui::Key;

#[test]
fn the_top_bar_renames_its_block() {
    let (mut test, client, block) = named_editor();
    assert_eq!(
        shown_name(&test),
        "",
        "an unnamed block leaves the field empty"
    );

    test.click("editor.name");
    test.text("Plans");
    test.key_press(Key::Enter);
    test.run();
    test.run();

    let named = name(&client, block).expect("the block took the typed name");
    assert!(named.manual, "a typed name is a manual one");
    assert_eq!(named.value, "Plans");
    assert_eq!(shown_name(&test), "Plans");
}
