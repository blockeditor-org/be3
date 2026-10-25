use block_editor_beui::be_block::be_model::BlockRef;
use block_editor_beui::be_block::{
    BlockContent, Home, HomeContent, SETTINGS, Settings, SettingsContent, WORKSPACE_TYPE,
};
use block_editor_beui::{BlockInfo, ChildContent};

use super::*;

#[test]
fn a_workspace_opens_on_its_landing_page() {
    let (mut fixture, landing) = editor();
    fixture.test.catalog(catalog());
    let (workspace, settings, home) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let store = fixture.test.store();
    let mut team = BlockInfo::new(workspace, WORKSPACE_TYPE, BlockParent::Root);
    team.metadata.insert(&SETTINGS, &BlockRef(settings));
    team.references = vec![settings, landing];
    store.add_block(team);
    store.add_block(BlockInfo::new(
        settings,
        SettingsContent::CONTENT_TYPE,
        BlockParent::Block(workspace),
    ));
    store.add_block(BlockInfo::new(
        home,
        HomeContent::CONTENT_TYPE,
        BlockParent::Block(settings),
    ));
    store.add_block(BlockInfo::new(landing, SHOWN_TYPE, BlockParent::Block(workspace)));
    let mut kinds = SettingsContent::default();
    kinds.apply(&Settings::set_kind(HomeContent::CONTENT_TYPE, home));
    fixture.test.hold(Some(settings), kinds);
    let mut kept = HomeContent::default();
    kept.apply(&Home::set_landing(Some(landing)));
    fixture.test.hold(Some(home), kept);
    fixture.settle();

    fixture.host.show_block(block_editor_beui::ShowRequest {
        block_id: workspace,
        block_type: WORKSPACE_TYPE,
        via: None,
        editor: None,
    });
    fixture.settle();
    let shown: Vec<Uuid> = fixture
        .test
        .children()
        .iter()
        .filter_map(|placement| match placement.content {
            ChildContent::Block { block_id, .. } => Some(Uuid::from_bytes(block_id)),
            _ => None,
        })
        .collect();
    assert!(
        shown.contains(&landing),
        "opening the workspace opens its landing page, showed {shown:?}"
    );
}
