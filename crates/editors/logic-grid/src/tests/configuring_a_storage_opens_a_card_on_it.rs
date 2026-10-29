use super::*;
use block_editor_beui::beui::Modifiers;

#[test]
fn configuring_a_storage_opens_a_card_on_it() {
    let mut editor = editor();
    editor.key(Key::Five, Modifiers::NONE);
    editor.run();
    editor.key(Key::One, Modifiers::NONE);
    editor.run();
    editor.key(Key::BracketRight, Modifiers::NONE);
    editor.run();
    let point = canvas_point(&editor, Vec2::new(-60.0, -80.0));
    editor.click(point);
    editor.run();
    assert_eq!(grid(&editor).components().count(), 1, "the click placed a storage");

    editor.key(Key::Five, Modifiers::NONE);
    editor.run();
    editor.key(Key::Two, Modifiers::NONE);
    editor.run();
    editor.click(point);
    editor.run();

    assert!(
        editor.shown("logic-grid.storage-bit.0"),
        "clicking a storage with the configure tool opens its bits"
    );
    let bit = editor.rect_of("logic-grid.storage-bit.0");
    assert!(
        bit.top() > point.y,
        "the card hangs below the storage it configures: {bit:?} under {point:?}"
    );
    editor.snapshot("configuring_a_storage_opens_a_card_on_it");
}
