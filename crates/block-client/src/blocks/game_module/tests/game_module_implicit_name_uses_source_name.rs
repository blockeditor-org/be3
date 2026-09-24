use block::Block;

use super::{GameModule, wasm_bytes};

#[test]
fn game_module_implicit_name_uses_source_name() {
    let named = GameModule::new("tic_tac_toe.wasm", wasm_bytes());
    let unnamed = GameModule::new("  ", wasm_bytes());

    assert_eq!(named.implicit_name(), Some("tic_tac_toe.wasm".to_owned()));
    assert_eq!(unnamed.implicit_name(), None);
}
