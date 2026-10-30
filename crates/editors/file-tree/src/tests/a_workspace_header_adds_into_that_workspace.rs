use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{
    AccessLevel, BlockCommand, BlockInfo, BlockParent, BlockQuery, ChildContent, PickedBlock,
};
use workspace_ui_types::WorkspaceContent;

use super::*;

#[test]
fn a_workspace_header_adds_into_that_workspace() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let files = my_files(&mut fixture, Vec::new());
    let team = Uuid::from_u128(4);
    let mut viewed = BlockInfo::new(team, WorkspaceContent::CONTENT_TYPE, BlockParent::Root);
    viewed.set_name("Team");
    viewed.author = Some(Uuid::from_u128(9));
    viewed.access = AccessLevel::View;
    let own = files_workspace(&fixture.host, &[]);
    fixture
        .host
        .set_blocks(BlockQuery::Roots, vec![own, viewed]);
    fixture
        .host
        .set_blocks(BlockQuery::References(team), Vec::new());
    fixture.settle();

    fixture.choose(files, "Add");
    let picker = fixture
        .test
        .placed_picker()
        .expect("the header's add opens a picker");
    let ChildContent::Picker { filter, .. } = picker.content else {
        unreachable!("placed_picker finds a picker")
    };
    assert!(
        filter.excluded.contains(&files.into_bytes()),
        "a workspace is not offered as a block to add to itself"
    );
    assert_eq!(
        filter.place,
        Some(BlockParent::Block(files).encode()),
        "the picker makes what it creates or imports in the workspace itself"
    );
    let row = fixture.test.rect_of(&format!("file-tree.{files}.row"));
    assert!(
        picker.rect.min.y >= row.max.y - 1.0 && picker.rect.min.x <= row.min.x + 1.0,
        "the picker hangs under the row whose add asked: {:?} under {row:?}",
        picker.rect
    );
    let picked = Uuid::from_u128(11);
    let text_type = Uuid::from_u128(7);
    fixture.test.answer_picker(Ok(Some(PickedBlock {
        block_id: picked.into_bytes(),
        block_type: text_type.into_bytes(),
        linked: false,
        placed: false,
    })));
    fixture.settle();
    assert!(
        fixture.test.placed_picker().is_none(),
        "the answered picker closes"
    );
    let placed = fixture.test.take_block_commands();
    assert!(
        placed.iter().any(|(id, command)| *id == picked
            && matches!(command, BlockCommand::Place { parent, .. } if *parent == files.into_bytes())),
        "the picked block is placed in the workspace whose header was pressed: {placed:?}"
    );
}
