use super::*;

use block::Block;

#[test]
fn stores_nothing() {
    let counter = Counter::new();
    assert!(counter.references().is_empty());
    assert_eq!(counter.implicit_name(), None);
    assert_eq!(serde_json::to_string(&counter).unwrap(), "{}");
}
