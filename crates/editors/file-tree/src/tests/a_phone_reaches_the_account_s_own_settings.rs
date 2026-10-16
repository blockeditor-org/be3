use block_editor_beui::be_block::{BlockContent, SettingsContent};
use block_editor_beui::{AccessLevel, BlockInfo, BlockParent};

use super::*;

#[test]
fn a_phone_reaches_the_account_s_own_settings() {
    let mut fixture = phone();
    let (mine, theirs) = (Uuid::from_u128(3), Uuid::from_u128(4));
    let mut own = BlockInfo::new(mine, SettingsContent::CONTENT_TYPE, BlockParent::Root);
    own.author = Some(fixture.host.account_id());
    own.set_intrinsic_name("Settings");
    let mut other = BlockInfo::new(theirs, SettingsContent::CONTENT_TYPE, BlockParent::Root);
    other.author = Some(Uuid::from_u128(9));
    other.access = AccessLevel::View;
    let workspace = files_workspace(&fixture.host, &[]);
    fixture.host.set_blocks(
        block_editor_beui::BlockQuery::Roots,
        vec![other, own, workspace],
    );
    fixture.settle();
    assert!(!fixture.test.shown(&format!("file-tree.{mine}.open")));

    fixture.test.click("file-tree.settings");
    fixture.settle();
    assert!(fixture.test.shown(&format!("file-tree.{mine}.open")));
    assert!(
        !fixture.test.shown(&format!("file-tree.{theirs}.open")),
        "another account's settings are never listed"
    );
    fixture
        .test
        .snapshot("a_phone_reaches_the_account_s_own_settings");

    fixture.test.click(&format!("file-tree.{mine}.open"));
    fixture.settle();
    assert_eq!(
        fixture.opened(),
        vec![mine],
        "tapping the settings block opens it"
    );
}
