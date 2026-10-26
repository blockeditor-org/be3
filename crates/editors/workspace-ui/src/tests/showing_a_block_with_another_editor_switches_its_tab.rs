use super::*;

const BUILT_IN: Uuid = Uuid::from_u128(1);
const DEBUG: Uuid = Uuid::from_u128(2);

fn shown_editors(fixture: &Fixture) -> Vec<Option<Uuid>> {
    fixture
        .test
        .children()
        .iter()
        .filter_map(|placement| match placement.content {
            ChildContent::Block { editor, .. } => Some(editor.map(Uuid::from_bytes)),
            _ => None,
        })
        .collect()
}

fn with_editor(fixture: &mut Fixture, id: Uuid, editor: Uuid) {
    fixture.host.show_block(block_editor_beui::ShowRequest {
        block_id: id,
        block_type: SHOWN_TYPE,
        via: None,
        editor: Some(editor),
    });
    fixture.settle();
}

#[test]
fn showing_a_block_with_another_editor_switches_its_tab() {
    let (mut fixture, opened) = editor();
    let mut catalog = catalog();
    catalog.editors = vec![
        block_editor_beui::EditorDescriptor {
            editor: BUILT_IN.into_bytes(),
            edits: block_editor_beui::Edits::Type(SHOWN_TYPE.into_bytes()),
            name: "Shown".to_owned(),
            icon_codepoint: String::new(),
            extensions: Vec::new(),
        },
        block_editor_beui::EditorDescriptor {
            editor: DEBUG.into_bytes(),
            edits: block_editor_beui::Edits::Any,
            name: "Debug view".to_owned(),
            icon_codepoint: String::new(),
            extensions: Vec::new(),
        },
    ];
    fixture.test.catalog(catalog);

    show(&mut fixture, opened, None);
    assert_eq!(
        shown_editors(&fixture),
        vec![Some(BUILT_IN)],
        "a block opens in its type's own editor"
    );

    with_editor(&mut fixture, opened, DEBUG);
    assert_eq!(fixture.open_tabs(), 1, "the block keeps its one tab");
    assert_eq!(shown_editors(&fixture), vec![Some(DEBUG)]);

    show(&mut fixture, opened, None);
    assert_eq!(
        shown_editors(&fixture),
        vec![Some(DEBUG)],
        "showing the block again keeps the editor its tab was switched to"
    );

    with_editor(&mut fixture, opened, BUILT_IN);
    assert_eq!(shown_editors(&fixture), vec![Some(BUILT_IN)]);
}
