use block_editor_beui::{
    BlockInfo, BlockParent, BlockTypeDescriptor, Catalog, ChildOperations, EditorDescriptor,
};

use super::*;

#[test]
fn open_with_offers_the_programs_that_open_a_files_extension() {
    let mut fixture = editor();
    let file_type = Uuid::from_u128(1);
    let (book, sketch) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let (reader, viewer) = (Uuid::from_u128(4), Uuid::from_u128(5));
    let program = |editor: Uuid, name: &str, extensions: &[&str]| EditorDescriptor {
        editor: editor.into_bytes(),
        edits: block_editor_beui::Edits::Any,
        name: name.to_owned(),
        icon_codepoint: String::new(),
        extensions: extensions
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
    };
    fixture.test.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: file_type.into_bytes(),
            display_name: "File".to_owned(),
            color: None,
            icon_codepoint: String::new(),
            children: ChildOperations::default(),
            press: block_editor_beui::Press::Open,
            plain_file: true,
        }],
        templates: Vec::new(),
        editors: vec![
            program(reader, "KOReader", &["epub", "pdf"]),
            program(viewer, "Image Viewer", &["png"]),
        ],
    });
    let mut listed_book = BlockInfo::new(book, file_type, BlockParent::Block(MY_FILES));
    listed_book.set_intrinsic_name("Moby Dick.EPUB");
    let mut listed_sketch = BlockInfo::new(sketch, file_type, BlockParent::Block(MY_FILES));
    listed_sketch.set_intrinsic_name("sketch.paint");
    my_files(&mut fixture, vec![listed_book, listed_sketch]);
    fixture.settle();

    fixture.choose(sketch, "Open with");
    let document = fixture.test.document();
    assert!(
        document
            .root()
            .and_then(|root| open_item(&document, root, "KOReader", false))
            .is_none(),
        "a program is not offered for a file it does not read"
    );
    drop(document);
    fixture
        .test
        .key(block_editor_beui::beui::Key::Escape, Modifiers::NONE);
    fixture.settle();

    fixture.choose(book, "Open with");
    fixture
        .test
        .snapshot("open_with_offers_the_programs_that_open_a_files_extension");
    let document = fixture.test.document();
    assert!(
        document
            .root()
            .and_then(|root| open_item(&document, root, "Image Viewer", false))
            .is_none(),
        "only the programs for the file's extension are offered"
    );
    let item = document
        .root()
        .and_then(|root| open_item(&document, root, "KOReader", false))
        .expect("Open with offers the program whose extensions name the file");
    drop(document);
    fixture.test.click(item);
    fixture.settle();

    let opens = fixture.test.take_opens();
    assert_eq!(opens.len(), 1);
    assert_eq!(opens[0].block_id, book);
    assert_eq!(opens[0].editor, Some(reader));
}
