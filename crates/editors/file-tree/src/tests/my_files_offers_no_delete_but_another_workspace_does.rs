use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::Key;
use block_editor_beui::{AccessLevel, BlockInfo, BlockParent, BlockQuery};
use workspace_ui_types::WorkspaceContent;

use super::*;

#[test]
fn my_files_offers_no_delete_but_another_workspace_does() {
    let mut fixture = editor();
    let mine = block_editor_beui::be_block::my_files(
        fixture.host.organization_id(),
        fixture.host.account_id(),
    );
    let mut files = files_workspace(&fixture.host, &[]);
    files.id = mine;
    files.access = AccessLevel::Manage;
    let team = Uuid::from_u128(5);
    let mut other = BlockInfo::new(team, WorkspaceContent::CONTENT_TYPE, BlockParent::Root);
    other.set_name("Team");
    other.author = Some(fixture.host.account_id());
    other.access = AccessLevel::Manage;
    fixture
        .host
        .set_blocks(BlockQuery::Roots, vec![files, other]);
    fixture.settle();
    for block in [mine, team] {
        fixture
            .host
            .set_blocks(BlockQuery::References(block), Vec::new());
    }
    fixture.settle();

    fixture.open_menu(mine);
    assert!(fixture.offers("Rename"), "My files' menu is open");
    assert!(
        !fixture.offers("Delete"),
        "deleting My files would only bring it back, so it is not offered"
    );
    fixture
        .test
        .snapshot("my_files_offers_no_delete_but_another_workspace_does");

    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    fixture.open_menu(team);
    assert!(fixture.offers("Delete"), "another workspace can be deleted");
}
