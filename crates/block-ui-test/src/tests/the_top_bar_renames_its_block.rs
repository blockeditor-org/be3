use super::*;

use beui::Key;

#[test]
fn the_top_bar_renames_its_block() {
    let (mut test, store, block) = named_editor();
    assert_eq!(
        shown_name(&test),
        "",
        "an unnamed block leaves the field empty"
    );

    test.click("editor.name");
    test.text("Plans");
    test.key_press(Key::Enter);
    settle(&mut test, &store);
    settle(&mut test, &store);

    let named = name(&store, block).expect("the block took the typed name as a manual one");
    assert_eq!(named, "Plans");
    assert_eq!(shown_name(&test), "Plans");
}
