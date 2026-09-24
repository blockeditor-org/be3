use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::game_module::GameModule;
use block_editor_plugin::be_block::GameModuleContent;
use block_editor_plugin::{Editor, EditorHost, PickedFile};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::{GameModuleApp, imported};

mod a_file_that_is_not_a_game_module_is_refused;
mod a_module_that_will_not_load_says_why;
mod the_replace_panel_goes_away_with_the_chrome;

const ACCOUNT: Uuid = Uuid::from_u128(0x6761_6d65_2d74_6573_742d_6163_636f_756e);
const WORKSPACE: Uuid = Uuid::from_u128(0x6761_6d65_2d74_6573_742d_776f_726b_7370);

fn editor(module: Vec<u8>) -> (ContentHarness<GameModuleApp>, Editor) {
    let client = Arc::new(BlockClient::new(ACCOUNT, WORKSPACE));
    let block = client.create_block(GameModule::new());
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(ACCOUNT);
    let editor = Editor::new(host.clone(), client, block.id());
    let mut test = ContentHarness::new(BeuiTest::new(editor.clone()), host);
    test.hold(None, GameModuleContent::from_file("game.wasm", module));
    test.run();
    (test, editor)
}
