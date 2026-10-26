use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{
    AccessLevel, AdministratorWatch, BlockInfo, BlockParent, BlockQuery, ShellCommand,
};
use workspace_ui_types::WorkspaceContent;

use super::*;

#[test]
fn a_block_whose_key_has_not_arrived_shows_locked_and_a_shared_workspace_can_be_left() {
    let mut fixture = editor();
    let text_type = Uuid::from_u128(7);
    let (notes, sealed, team) = (Uuid::from_u128(3), Uuid::from_u128(4), Uuid::from_u128(5));
    let mut written = BlockInfo::new(notes, text_type, BlockParent::Block(MY_FILES));
    written.set_intrinsic_name("Notes");
    let mut locked = BlockInfo::new(sealed, text_type, BlockParent::Block(MY_FILES));
    locked.locked = true;
    locked.access = AccessLevel::View;
    let inside = vec![written, locked];
    let workspace = files_workspace(&fixture.host, &inside);
    let mut shared_space = BlockInfo::new(team, WorkspaceContent::CONTENT_TYPE, BlockParent::Root);
    shared_space.set_name("Team");
    shared_space.author = Some(Uuid::from_u128(9));
    shared_space.access = AccessLevel::View;
    fixture
        .host
        .set_blocks(BlockQuery::Roots, vec![shared_space, workspace]);
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(MY_FILES), inside);
    fixture
        .host
        .set_blocks(BlockQuery::References(team), Vec::new());
    fixture.settle();

    assert!(fixture.says("Locked"), "a block it cannot open yet says so");
    fixture.test.snapshot(
        "a_block_whose_key_has_not_arrived_shows_locked_and_a_shared_workspace_can_be_left",
    );

    fixture.choose(team, "Leave");
    assert!(
        fixture
            .test
            .take_shell_commands()
            .contains(&(team, ShellCommand::Leave)),
        "leaving a workspace someone shared asks the host to take this account off it"
    );

    fixture.test.set_watched::<AdministratorWatch>(&true);
    fixture.settle();
    fixture.open_menu(team);
    assert!(fixture.offers("Rename"), "the workspace's menu is open");
    assert!(
        !fixture.offers("Leave"),
        "an administrator keeps every block whatever it leaves, so it is not offered"
    );
}
