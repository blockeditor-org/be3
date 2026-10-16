use super::*;
use block_editor_beui::be_block::{InputRemapper, Root};
use block_editor_beui::{Catalog, EditorDescriptor};

#[test]
fn choosing_a_remapper_makes_a_block_of_it_and_opens_it() {
    let mut editor = editor(vec![mouse()]);
    editor.set_size(Vec2::new(800.0, 900.0));
    let remapper = Uuid::new_v4();
    editor.catalog(Catalog {
        types: Vec::new(),
        templates: Vec::new(),
        editors: vec![EditorDescriptor {
            editor: *remapper.as_bytes(),
            block_type: *InputRemapper::CONTENT_TYPE.as_bytes(),
            name: "Input Remaps".to_owned(),
            icon_codepoint: String::new(),
        }],
    });

    editor.click("input-settings.remapper");
    editor.run();
    editor.key_press(Key::Home);
    editor.run();
    editor.key_press(Key::ArrowDown);
    editor.run();
    editor.key_press(Key::Enter);
    editor.run();

    let choice = content(&editor)
        .root()
        .remapper
        .expect("the remapper is chosen");
    assert_eq!(choice.editor, remapper);
    assert!(
        editor
            .seeded()
            .iter()
            .any(|seeded| seeded.block == choice.block),
        "a block of its remaps was made"
    );
    let opens = editor.take_opens();
    assert_eq!(opens.len(), 1);
    assert_eq!(opens[0].block_id, choice.block);
    assert_eq!(opens[0].editor, Some(remapper));
    editor.snapshot("choosing_a_remapper_makes_a_block_of_it_and_opens_it");

    editor.click("input-settings.remapper.open");
    editor.run();
    assert_eq!(editor.take_opens().len(), 1, "Open opens it again");
}
