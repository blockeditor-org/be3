use be_sdk_beui::be_block::{SettingsProfileContent, VIEW_TYPE};
use be_sdk_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_block_only_an_open_tab_shows_is_not_used_anywhere() {
    use be_sdk_beui::be_block::BlockContent;
    let mut fixture = editor();
    my_files(&mut fixture, Vec::new());
    fixture.settle();
    let mut note = BlockInfo::new(Uuid::from_u128(0x20), PLAIN, BlockParent::Unfiled);
    note.set_name("Note");
    let mut tab = BlockInfo::new(Uuid::from_u128(0x30), VIEW_TYPE, BlockParent::Detached);
    tab.set_name("Tab");
    let mut profile = BlockInfo::new(
        Uuid::from_u128(0x40),
        SettingsProfileContent::CONTENT_TYPE,
        BlockParent::Detached,
    );
    profile.set_name("Laptop");
    fixture.test.click("file-tree.unfiled.chevron");
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::Unfiled, vec![note.clone()]);
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::Backrefs(note.id), vec![tab, profile]);
    fixture.settle();

    assert!(
        fixture.says("Not used anywhere"),
        "an open tab and the account's own settings show the block, they do not use it"
    );
    assert!(!fixture.says("used in Tab"));
}
