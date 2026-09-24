use std::sync::Arc;

use block::Block as _;
use block_client::BlockClient;
use block_client::blocks::deterministic_game::DeterministicGame as GameBlock;
use block_client::blocks::game_module::GameModule;
use block_editor_plugin::be_block::{DeterministicGame, DeterministicGameContent, GameModuleContent};
use block_editor_plugin::{Creation, Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::{DeterministicGameApp, module_filter};

mod a_module_that_is_not_a_game_is_reported;
mod the_creation_dialog_is_drawn_with_beui;
mod the_picker_asks_only_for_game_modules;

const ACCOUNT: Uuid = Uuid::from_u128(0x6465_742d_7465_7374_2d61_6363_6f75_6e74);
const WORKSPACE: Uuid = Uuid::from_u128(0x6465_742d_7465_7374_2d77_6f72_6b73_7061);

fn editor(module: Vec<u8>) -> ContentHarness<DeterministicGameApp> {
    let client = Arc::new(BlockClient::new(ACCOUNT, WORKSPACE));
    let module_block = client.create_block(GameModule::new());
    let block = client.create_block(GameBlock::with_references(vec![module_block.id()]));
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(ACCOUNT);
    let editor = BeuiTest::new(Editor::new(host.clone(), client, block.id()));
    let mut harness = ContentHarness::new(editor, host);
    harness.hold(
        None,
        DeterministicGameContent::new(&DeterministicGame::of(module_block.id())),
    );
    harness.hold(
        Some(module_block.id()),
        GameModuleContent::from_file("game.wasm", module),
    );
    harness.run();
    harness.run();
    harness
}

fn creation_editor() -> BeuiTest<DeterministicGameApp> {
    let client = Arc::new(BlockClient::new(ACCOUNT, WORKSPACE));
    let host = EditorHost::default();
    BeuiTest::creation(Creation::new(host, client))
}
