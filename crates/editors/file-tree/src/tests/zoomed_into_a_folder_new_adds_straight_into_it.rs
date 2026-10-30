use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery, ChildContent, PickedBlock};
use workspace_ui_types::FolderContent;

use super::*;

#[test]
fn zoomed_into_a_folder_new_adds_straight_into_it() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let (folder, roadmap) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let mut projects = BlockInfo::new(
        folder,
        FolderContent::CONTENT_TYPE,
        BlockParent::Block(MY_FILES),
    );
    projects.set_intrinsic_name("Projects".to_owned());
    projects.references = vec![roadmap];
    my_files(&mut fixture, vec![projects.clone()]);
    fixture.settle();
    let mut road = BlockInfo::new(roadmap, Uuid::from_u128(7), BlockParent::Block(folder));
    road.set_intrinsic_name("Roadmap".to_owned());

    fixture.test.click(format!("file-tree.{folder}.row"));
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), vec![road.clone()]);
    fixture.settle();
    assert!(fixture.test.shown(&format!("file-tree.{roadmap}.row")));
    assert!(fixture.test.shown("file-tree.back"));
    assert!(
        fixture.test.shown(&format!("file-tree.{MY_FILES}.crumb")),
        "the location line lists the folder's ancestors"
    );
    assert_eq!(
        fixture.test.accessible_label("file-tree.new").as_deref(),
        Some("New")
    );
    fixture
        .test
        .snapshot("zoomed_into_a_folder_new_is_a_primary_button");

    fixture.test.click("file-tree.new");
    fixture.settle();
    let picker = fixture
        .test
        .placed_picker()
        .expect("New opens the picker straight into the folder, with no menu");
    let ChildContent::Picker { filter, .. } = picker.content else {
        unreachable!("placed_picker finds a picker")
    };
    assert_eq!(filter.place, Some(BlockParent::Block(folder).encode()));
    let made = Uuid::from_u128(11);
    fixture.test.answer_picker(Ok(Some(PickedBlock {
        block_id: made.into_bytes(),
        block_type: Uuid::from_u128(7).into_bytes(),
        linked: false,
        placed: false,
    })));
    fixture.settle();
    let mut untitled = BlockInfo::new(made, Uuid::from_u128(7), BlockParent::Block(folder));
    untitled.set_intrinsic_name("Untitled".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), vec![road, untitled]);
    fixture.settle();
    assert!(
        fixture.test.shown(&format!("file-tree.{made}.rename")),
        "the new block is named in place"
    );

    fixture.test.click(format!("file-tree.{MY_FILES}.crumb"));
    fixture.settle();
    assert!(
        fixture.test.shown(&format!("file-tree.{folder}.row")),
        "pressing an ancestor in the location line zooms out to it"
    );
    assert!(fixture.test.shown("file-tree.back"));
}
