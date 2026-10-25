use block_editor_beui::be_block::be_model::BlockRef;
use block_editor_beui::be_block::{
    BlockContent, Home, HomeContent, SETTINGS, Settings, SettingsContent,
};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_pinned_block_is_listed_first_in_its_workspace() {
    let mut fixture = editor();
    let (notes, plans, settings, home) = (
        Uuid::from_u128(5),
        Uuid::from_u128(6),
        Uuid::from_u128(7),
        Uuid::from_u128(8),
    );
    let mut listed = BlockInfo::new(notes, Uuid::from_u128(1), BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    let mut pinned = BlockInfo::new(plans, Uuid::from_u128(1), BlockParent::Block(MY_FILES));
    pinned.set_name("Plans");
    let mut files = files_workspace(&fixture.host, &[listed.clone(), pinned.clone()]);
    files.metadata.insert(&SETTINGS, &BlockRef(settings));
    fixture.host.set_blocks(BlockQuery::Roots, vec![files.clone()]);
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(MY_FILES), vec![listed, pinned.clone()]);
    fixture
        .host
        .set_blocks(BlockQuery::Block(plans), vec![pinned]);
    let mut kinds = SettingsContent::default();
    kinds.apply(&Settings::set_kind(HomeContent::CONTENT_TYPE, home));
    fixture.test.hold(Some(settings), kinds);
    let mut kept = HomeContent::default();
    kept.apply(&Home::pin(plans, true));
    fixture.test.hold(Some(home), kept);
    fixture.settle();

    assert!(fixture.says("Pinned"), "the workspace lists what is pinned first");
    assert!(
        fixture.test.shown(&format!("file-tree.pinned.{plans}.row")),
        "the pinned block has a row of its own under Pinned"
    );
    fixture
        .test
        .snapshot("a_pinned_block_is_listed_first_in_its_workspace");
}
