use super::*;
use block_editor_beui::be_block::{InputRemapper, Root};
use block_editor_beui::beui::Vec2;
use block_editor_beui::{Catalog, EditorDescriptor};

#[test]
fn opening_the_remaps_makes_them_with_the_remapper_and_opens_them() {
    let mut editor = editor(vec![mouse()]);
    editor.set_size(Vec2::new(800.0, 1400.0));
    let remapper = Uuid::new_v4();
    editor.catalog(Catalog {
        types: Vec::new(),
        templates: Vec::new(),
        editors: vec![EditorDescriptor {
            editor: *remapper.as_bytes(),
            edits: block_editor_beui::Edits::Type(*InputRemapper::CONTENT_TYPE.as_bytes()),
            name: "Input Remaps".to_owned(),
            icon_codepoint: String::new(),
            extensions: Vec::new(),
        }],
    });

    editor.click("input.remaps");
    editor.run();

    let choice = content(&editor)
        .root()
        .remapper
        .expect("opening the remaps chose the only remapper");
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
    editor.snapshot("opening_the_remaps_makes_them_with_the_remapper_and_opens_them");

    editor.click("input.remaps");
    editor.run();
    let opens = editor.take_opens();
    assert_eq!(opens.len(), 1);
    assert_eq!(
        opens[0].block_id, choice.block,
        "opening them again opens the same block"
    );
    assert_eq!(editor.seeded().len(), 1);

    reset(&mut editor, "input.remapper");
    assert!(
        content(&editor).root().remapper.is_some(),
        "resetting asks first"
    );
    editor.snapshot("resetting_the_remaps_asks_before_deleting_them");
    editor.click("input.remapper.reset.confirm");
    editor.run();
    assert_eq!(content(&editor).root().remapper, None);
    assert!(
        !content(&editor).root().references().contains(&choice.block),
        "the remaps' block is no longer listed, so it is in the trash"
    );
}
