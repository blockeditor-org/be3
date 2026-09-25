use block_editor_plugin::be_block::BlockContent;

use block_editor_plugin::be_block::{
    DeterministicGame, DeterministicGameContent, GameModuleContent,
};
use block_editor_plugin::{Creation, Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::{DeterministicGameApp, module_filter};

mod a_module_that_is_not_a_game_is_reported;
mod the_creation_dialog_is_drawn_with_beui;
mod the_picker_asks_only_for_game_modules;

const ACCOUNT: Uuid = Uuid::from_u128(0x6465_742d_7465_7374_2d61_6363_6f75_6e74);

fn editor(module: Vec<u8>) -> ContentHarness<DeterministicGameApp> {
    let module_block = Uuid::new_v4();
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_account_id(ACCOUNT);
    let editor = BeuiTest::new(Editor::new(host.clone(), block));
    let mut harness = ContentHarness::new(editor, host);
    harness.hold(
        None,
        DeterministicGameContent::new(&DeterministicGame::of(module_block)),
    );
    harness.hold(
        Some(module_block),
        GameModuleContent::from_file("game.wasm", module),
    );
    harness.run();
    harness.run();
    harness
}

fn creation_editor() -> BeuiTest<DeterministicGameApp> {
    let host = EditorHost::default();
    BeuiTest::creation(Creation::new(host))
}
