use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::Key;
use block_editor_beui::{AccessLevel, BlockCommand, BlockInfo, BlockParent, BlockQuery};
use block_ui_test::{HostCall, PickedBlock};
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

    let add = format!("file-tree.{files}.add");
    assert!(
        !fixture.test.shown("file-tree.add-root"),
        "the toolbar has no add button of its own"
    );
    assert!(fixture.test.shown("file-tree.new-workspace"));
    assert_eq!(
        fixture.test.accessible_label(&add).as_deref(),
        Some("Add to My files")
    );
    assert!(
        !fixture.test.shown(&format!("file-tree.{team}.add")),
        "a workspace the account may only view offers no add"
    );

    fixture.test.hover(format!("file-tree.{files}.row"));
    fixture.settle();
    fixture
        .test
        .snapshot("a_workspace_header_adds_into_that_workspace");

    fixture.test.click(&add);
    fixture.settle();
    let asked = fixture.test.take_requests();
    let [
        HostCall::PickBlock {
            reply: pick,
            filter,
        },
    ]: [_; 1] = asked.try_into().unwrap_or_else(|asked: Vec<_>| {
        panic!("the header's add asked for {asked:?} rather than a block")
    })
    else {
        panic!("the header's add asked for something other than a block");
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
    let picked = Uuid::from_u128(11);
    let text_type = Uuid::from_u128(7);
    fixture.test.answer(
        pick,
        Ok(Some(PickedBlock {
            block_id: picked.into_bytes(),
            block_type: text_type.into_bytes(),
            linked: false,
            placed: false,
        })),
    );
    fixture.settle();
    let placed = fixture.test.take_block_commands();
    assert!(
        placed.iter().any(|(id, command)| *id == picked
            && matches!(command, BlockCommand::Place { parent, .. } if *parent == files.into_bytes())),
        "the picked block is placed in the workspace whose header was pressed: {placed:?}"
    );

    for _ in 0..2 {
        fixture.test.click(format!("file-tree.{files}.chevron"));
        fixture.settle();
    }
    fixture.test.key(Key::Tab, Modifiers::NONE);
    fixture.settle();
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    let asked = fixture.test.take_requests();
    assert!(
        matches!(asked.as_slice(), [HostCall::PickBlock { .. }]),
        "Tab from a workspace's header reaches its add, and Enter presses it: {asked:?}"
    );
}
