use block_editor_beui::be_block::{BlockContent, SETTINGS, SettingsContent};
use block_editor_beui::{BlockInfo, BlockParent};
use logic_grid_types::HotbarContent;

use super::*;

#[test]
fn a_manager_gives_the_world_a_hotbar_that_players_start_from() {
    let mut editor = editor();
    let world = editor.block_id().expect("the game is a block");
    editor.store().add_block(BlockInfo::new(
        world,
        LogicGameContent::CONTENT_TYPE,
        BlockParent::Root,
    ));
    editor.run();
    editor.snapshot("a_manager_gives_the_world_a_hotbar_that_players_start_from");

    editor.click("logic-game.world-hotbar");
    let store = editor.store();
    let created = store.created();
    let settings = created
        .iter()
        .copied()
        .find(|block| {
            store
                .block(*block)
                .is_some_and(|info| info.block_type == SettingsContent::CONTENT_TYPE)
        })
        .expect("the world gets settings of its own");
    assert_eq!(
        store.block(settings).and_then(|info| info.parent),
        Some(BlockParent::Block(world))
    );
    assert_eq!(
        store
            .block(world)
            .and_then(|info| info.metadata.get(&SETTINGS))
            .map(|found| found.0),
        Some(settings)
    );
    let hotbar = created
        .iter()
        .copied()
        .find(|block| {
            store
                .block(*block)
                .is_some_and(|info| info.block_type == HotbarContent::CONTENT_TYPE)
        })
        .expect("the world's hotbar is made");
    assert_eq!(
        store.block(hotbar).and_then(|info| info.parent),
        Some(BlockParent::Block(settings))
    );
    assert_eq!(
        editor
            .take_opens()
            .into_iter()
            .map(|open| open.block_id)
            .collect::<Vec<_>>(),
        [hotbar]
    );
}
