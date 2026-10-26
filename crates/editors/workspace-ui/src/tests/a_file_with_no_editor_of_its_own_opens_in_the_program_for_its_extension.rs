use super::*;

const READER: Uuid = Uuid::from_u128(0x5ead);
const BOOK_TYPE: Uuid = Uuid::from_u128(0xb00c);

fn shown(fixture: &Fixture, id: Uuid) -> Vec<Option<Uuid>> {
    fixture
        .test
        .children()
        .iter()
        .filter_map(|placement| match placement.content {
            ChildContent::Block {
                block_id, editor, ..
            } if Uuid::from_bytes(block_id) == id => Some(editor.map(Uuid::from_bytes)),
            _ => None,
        })
        .collect()
}

fn open(fixture: &mut Fixture, id: Uuid) {
    fixture.host.show_block(block_editor_beui::ShowRequest {
        block_id: id,
        block_type: BOOK_TYPE,
        via: None,
        editor: None,
    });
    fixture.settle();
}

#[test]
fn a_file_with_no_editor_of_its_own_opens_in_the_program_for_its_extension() {
    let (mut fixture, _) = editor();
    let mut catalog = catalog();
    catalog.types.push(block_editor_beui::BlockTypeDescriptor {
        block_type: BOOK_TYPE.into_bytes(),
        display_name: "File".to_owned(),
        color: None,
        icon_codepoint: String::new(),
        children: block_plugin_api::ChildOperations::default(),
        press: block_plugin_api::Press::Open,
        plain_file: true,
    });
    catalog.editors = vec![block_editor_beui::EditorDescriptor {
        editor: READER.into_bytes(),
        edits: block_editor_beui::Edits::Any,
        name: "KOReader".to_owned(),
        icon_codepoint: String::new(),
        extensions: vec!["epub".to_owned(), "pdf".to_owned()],
    }];
    fixture.test.catalog(catalog);
    let (book, paint) = (Uuid::new_v4(), Uuid::new_v4());
    let store = fixture.test.store();
    let mut info = block_editor_beui::BlockInfo::new(book, BOOK_TYPE, BlockParent::Root);
    info.set_intrinsic_name("Moby Dick.epub");
    info.set_name("Moby Dick");
    store.add_block(info);
    let mut info = block_editor_beui::BlockInfo::new(paint, BOOK_TYPE, BlockParent::Root);
    info.set_intrinsic_name("sketch.paint");
    store.add_block(info);
    fixture.settle();

    open(&mut fixture, book);
    assert_eq!(
        shown(&fixture, book),
        vec![Some(READER)],
        "a book opens in the program whose extensions name the file it came from, though it was renamed"
    );

    open(&mut fixture, paint);
    assert_eq!(
        shown(&fixture, paint),
        vec![None],
        "a file no program opens has no editor"
    );
}
