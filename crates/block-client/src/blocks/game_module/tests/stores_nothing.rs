use super::*;

use block::Block;

#[test]
fn stores_nothing() {
    let game_module = GameModule::new();
    assert!(game_module.references().is_empty());
    assert_eq!(game_module.implicit_name(), None);
    assert_eq!(serde_json::to_string(&game_module).unwrap(), "{}");
}
