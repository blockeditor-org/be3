use block_editor_beui::be_block::{BlockContent, SettingsContent};
use block_editor_beui::{AccessLevel, BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn the_account_s_system_blocks_are_reached_from_the_bottom_of_the_files() {
    let mut fixture = editor();
    let (mine, theirs, profile) = (Uuid::from_u128(3), Uuid::from_u128(4), Uuid::from_u128(5));
    let mut own = BlockInfo::new(mine, SettingsContent::CONTENT_TYPE, BlockParent::Root);
    own.author = Some(fixture.host.account_id());
    own.set_intrinsic_name("Settings");
    own.references = vec![profile];
    let mut other = BlockInfo::new(theirs, SettingsContent::CONTENT_TYPE, BlockParent::Root);
    other.author = Some(Uuid::from_u128(9));
    other.access = AccessLevel::View;
    let mut device = BlockInfo::new(profile, Uuid::from_u128(7), BlockParent::Block(mine));
    device.set_name("Laptop");
    let workspace = files_workspace(&fixture.host, &[]);
    fixture
        .host
        .set_blocks(BlockQuery::Roots, vec![other, own, workspace]);
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(MY_FILES), Vec::new());
    fixture.settle();

    let top = |fixture: &Fixture, id: &str| fixture.test.rect_of(id).top();
    assert!(
        top(&fixture, "file-tree.orphans.row") < top(&fixture, "file-tree.system.row"),
        "System sits at the bottom, below Recently Deleted"
    );
    assert!(
        !fixture.test.shown(&format!("file-tree.{mine}.row")),
        "the system blocks are folded away until asked for"
    );

    fixture.test.click("file-tree.system.chevron");
    fixture.settle();
    fixture.test.click(format!("file-tree.{mine}.chevron"));
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(mine), vec![device]);
    fixture.settle();
    assert!(fixture.test.shown(&format!("file-tree.{mine}.row")));
    assert!(
        fixture.test.shown(&format!("file-tree.{profile}.row")),
        "a settings block opens to what it holds, its profiles among them"
    );
    assert!(
        !fixture.test.shown(&format!("file-tree.{theirs}.row")),
        "another account's settings are never listed"
    );
    fixture
        .test
        .snapshot("the_account_s_system_blocks_are_reached_from_the_bottom_of_the_files");

    fixture.test.click(format!("file-tree.{mine}.row"));
    fixture.settle();
    assert_eq!(
        fixture.opened(),
        vec![mine],
        "choosing the settings block opens it"
    );
}
