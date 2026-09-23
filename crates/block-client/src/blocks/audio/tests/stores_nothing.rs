use super::*;

use block::Block;

#[test]
fn stores_nothing() {
    let audio = Audio::new();
    assert!(audio.references().is_empty());
    assert_eq!(audio.implicit_name(), None);
    assert_eq!(serde_json::to_string(&audio).unwrap(), "{}");
}
