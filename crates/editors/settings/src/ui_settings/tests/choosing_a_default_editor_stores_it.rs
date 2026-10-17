use block_editor_beui::be_block::EditorSettingsContent;
use block_editor_beui::{BlockTypeDescriptor, Catalog, ChildOperations, EditorDescriptor};

use super::*;
use crate::ui_settings::DefaultEditorsApp;

#[test]
fn choosing_a_default_editor_stores_it() {
    let block_type = Uuid::from_u128(1);
    let (built_in, debug) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let host = EditorHost::default();
    host.set_editable(true);
    let mut editor = BeuiTest::<DefaultEditorsApp>::new(OwnedEditor::new(host, Uuid::from_u128(4)));
    editor.hold(None, EditorSettingsContent::default());
    editor.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: block_type.into_bytes(),
            display_name: "Note".to_owned(),
            icon_codepoint: String::new(),
            children: ChildOperations::default(),
        }],
        templates: Vec::new(),
        editors: vec![
            EditorDescriptor {
                editor: built_in.into_bytes(),
                block_type: block_type.into_bytes(),
                name: "Note".to_owned(),
                icon_codepoint: String::new(),
            },
            EditorDescriptor {
                editor: debug.into_bytes(),
                block_type: block_editor_beui::ANY_BLOCK_TYPE,
                name: "Debug view".to_owned(),
                icon_codepoint: String::new(),
            },
        ],
    });
    editor.run();
    let picker = format!("default-editors.{block_type}");
    let choose = |editor: &mut BeuiTest<DefaultEditorsApp>, index: usize| {
        editor.click(&picker);
        editor.run();
        editor.key_press(Key::Home);
        editor.run();
        for _ in 0..index {
            editor.key_press(Key::ArrowDown);
            editor.run();
        }
        editor.key_press(Key::Enter);
        editor.run();
    };
    let stored = |editor: &BeuiTest<DefaultEditorsApp>| {
        editor
            .content::<EditorSettingsContent>(None)
            .root()
            .default_editor(block_type)
    };

    choose(&mut editor, 2);
    assert_eq!(stored(&editor), Some(debug));
    editor.snapshot("choosing_a_default_editor_stores_it");

    choose(&mut editor, 0);
    assert_eq!(stored(&editor), None);
}
