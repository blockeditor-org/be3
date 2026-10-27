use block_editor_beui::{BlockTypeDescriptor, Catalog, ChildOperations, EditorDescriptor};
use block_ui_test::prelude::*;
use settings_types::EditorSettingsContent;

use block_editor_beui::beui::{Key, Modifiers};
use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::default_editors::DefaultEditorsApp;

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
            color: None,
            icon_codepoint: String::new(),
            children: ChildOperations::default(),
            press: block_editor_beui::Press::Open,
            plain_file: false,
        }],
        templates: Vec::new(),
        editors: vec![
            EditorDescriptor {
                editor: built_in.into_bytes(),
                edits: block_editor_beui::Edits::Type(block_type.into_bytes()),
                name: "Note".to_owned(),
                icon_codepoint: String::new(),
                extensions: Vec::new(),
            },
            EditorDescriptor {
                editor: debug.into_bytes(),
                edits: block_editor_beui::Edits::Any,
                name: "Debug view".to_owned(),
                icon_codepoint: String::new(),
                extensions: Vec::new(),
            },
        ],
    });
    editor.run();
    let picker = format!("default-editors.{block_type}");
    let choose = |editor: &mut BeuiTest<DefaultEditorsApp>, index: usize| {
        editor.click(&picker);
        editor.key(Key::Home, Modifiers::NONE);
        for _ in 0..index {
            editor.key(Key::ArrowDown, Modifiers::NONE);
        }
        editor.key(Key::Enter, Modifiers::NONE);
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
