use be_sdk_beui::{Editor, EditorHost, OwnedEditor, PickedFile};
use be_sdk_test::BeuiTest;
use be_type_deterministic_game::GameModuleFile;
use uuid::Uuid;

use crate::game_module::root::{GameModuleRoot, imported};

mod a_file_that_is_not_a_game_module_is_refused;
mod a_module_that_will_not_load_says_why;
mod the_replace_panel_goes_away_with_the_chrome;

const ACCOUNT: Uuid = Uuid::from_u128(0x6761_6d65_2d74_6573_742d_6163_636f_756e);

fn editor(module: Vec<u8>) -> (BeuiTest<GameModuleRoot>, Editor) {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(ACCOUNT);
    let owner = OwnedEditor::new(host.clone(), block);
    let editor = *owner;
    let mut test = BeuiTest::new(owner);
    test.hold(None, GameModuleFile::from_file("game.wasm", module));
    test.run();
    (test, editor)
}
