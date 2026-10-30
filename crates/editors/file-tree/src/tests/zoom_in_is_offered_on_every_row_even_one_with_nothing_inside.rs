use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{
    BlockInfo, BlockParent, BlockQuery, BlockTypeDescriptor, Catalog, ChildOperations, Press,
};
use workspace_ui_types::{FolderContent, WorkspaceContent};

use super::*;

#[test]
fn zoom_in_is_offered_on_every_row_even_one_with_nothing_inside() {
    let mut fixture = editor();
    let (canvas_type, note_type) = (Uuid::from_u128(20), Uuid::from_u128(21));
    let kind = |block_type: Uuid, name: &str, children: ChildOperations, press: Press| {
        BlockTypeDescriptor {
            block_type: block_type.into_bytes(),
            display_name: name.to_owned(),
            color: None,
            icon_codepoint: String::new(),
            children,
            press,
            plain_file: false,
        }
    };
    let holds = ChildOperations {
        add: true,
        delete: true,
        replace: true,
    };
    fixture.test.catalog(Catalog {
        types: vec![
            kind(
                WorkspaceContent::CONTENT_TYPE,
                "Workspace",
                holds,
                Press::Zoom,
            ),
            kind(FolderContent::CONTENT_TYPE, "Folder", holds, Press::Zoom),
            kind(canvas_type, "Canvas", holds, Press::Open),
            kind(note_type, "Note", ChildOperations::default(), Press::Open),
        ],
        templates: Vec::new(),
        editors: Vec::new(),
    });
    let (board, memo, archive, linked, kept) = (
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        Uuid::from_u128(4),
        Uuid::from_u128(5),
        Uuid::from_u128(6),
    );
    let mut canvas = BlockInfo::new(board, canvas_type, BlockParent::Block(MY_FILES));
    canvas.set_intrinsic_name("Board".to_owned());
    let mut note = BlockInfo::new(memo, note_type, BlockParent::Block(MY_FILES));
    note.set_intrinsic_name("Memo".to_owned());
    let mut folder = BlockInfo::new(archive, FolderContent::CONTENT_TYPE, BlockParent::Root);
    folder.set_intrinsic_name("Archive".to_owned());
    folder.references = vec![kept];
    let mut link = folder.clone();
    link.id = linked;
    link.set_intrinsic_name("Elsewhere".to_owned());
    let mut inside = BlockInfo::new(kept, note_type, BlockParent::Block(linked));
    inside.set_intrinsic_name("Kept".to_owned());
    my_files(&mut fixture, vec![canvas, note, link]);
    fixture.settle();

    fixture.open_menu(board);
    fixture.click_item("Zoom in");
    assert_eq!(
        fixture.test.label("file-tree.here"),
        "Board",
        "a canvas with nothing on it can still be zoomed into"
    );
    assert!(
        fixture.says("Nothing here yet"),
        "zoomed into a block that holds nothing, the files say so"
    );
    assert_eq!(
        fixture.test.accessible_label("file-tree.new").as_deref(),
        Some("New"),
        "New adds straight into the canvas, which can hold blocks"
    );
    fixture
        .test
        .snapshot("zoomed_into_a_canvas_with_nothing_on_it");

    let mut added = BlockInfo::new(Uuid::from_u128(11), note_type, BlockParent::Block(board));
    added.set_intrinsic_name("Sketch".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::References(board), vec![added.clone()]);
    fixture.settle();
    assert!(
        fixture.test.shown(&format!("file-tree.{}.row", added.id)),
        "what is added to the block zoomed into is listed at once"
    );

    fixture.test.click("file-tree.back");
    fixture.settle();
    fixture.open_menu(memo);
    fixture.click_item("Zoom in");
    assert_eq!(fixture.test.label("file-tree.here"), "Memo");
    assert!(fixture.says("Nothing here yet"));
    fixture.test.click("file-tree.new");
    fixture.settle();
    assert!(
        !fixture.offers("New workspace") && fixture.test.placed_picker().is_none(),
        "New does nothing inside a block that cannot hold others"
    );

    fixture.test.click("file-tree.back");
    fixture.settle();
    fixture.open_menu(linked);
    fixture.click_item("Zoom in");
    assert_eq!(fixture.test.label("file-tree.here"), "Elsewhere");
    fixture
        .host
        .set_blocks(BlockQuery::References(linked), vec![inside]);
    fixture.settle();
    assert!(
        fixture.test.shown(&format!("file-tree.{kept}.row")),
        "zoomed into a link, the files list what the linked block holds"
    );

    fixture.test.click("file-tree.back");
    fixture.settle();
    assert_eq!(fixture.test.label("file-tree.here"), "My files");
    fixture.test.click("file-tree.back");
    fixture.settle();
    assert!(!fixture.test.shown("file-tree.back"));
    fixture.open_menu(MY_FILES);
    fixture.click_item("Zoom in");
    assert_eq!(fixture.test.label("file-tree.here"), "My files");
}
