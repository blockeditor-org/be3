use be_sdk_beui::{Editor, EditorHost, OwnedEditor, PickedFile};
use be_sdk_test::BeuiTest;
use be_type_deterministic_game::GameModuleFile;
use uuid::Uuid;

use crate::game_module::root::{GameModuleRoot, imported};

mod a_file_that_is_not_a_game_module_is_refused;
mod a_module_that_will_not_load_says_why;
mod on_a_phone_the_replace_panel_is_gathered_beside_the_module;
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

/// `module` grown to exactly `len` bytes by a custom section, which loading
/// ignores. The module view shows its size, and a module the build compiled
/// changes size with the compiler, its flags, the build's configuration and
/// every dependency, so a painting of one pins its size here to change only
/// with the view.
fn sized(mut module: Vec<u8>, len: usize) -> Vec<u8> {
    const NAME: &[u8] = b"padding";
    let header = 1 + leb128(NAME.len()).len() + NAME.len();
    // The section is its id, its body's length as LEB128, then the body, so
    // try each length of that LEB128 for the one that adds up to `len`.
    let body = (1..=5)
        .filter_map(|length_bytes| len.checked_sub(module.len() + 1 + length_bytes))
        .find(|&body| module.len() + 1 + leb128(body).len() + body == len)
        .filter(|&body| body >= header)
        .unwrap_or_else(|| {
            panic!(
                "a module of {} bytes leaves no room for a section in {len}",
                module.len()
            )
        });
    module.push(0);
    module.extend(leb128(body));
    module.extend(leb128(NAME.len()));
    module.extend(NAME);
    module.resize(len, 0);
    module
}

fn leb128(mut value: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            bytes.push(byte);
            return bytes;
        }
        bytes.push(byte | 0x80);
    }
}
