use super::*;

use block::Block;

#[test]
fn stores_nothing() {
    let paint_snapshot = PaintSnapshot::new();
    assert!(paint_snapshot.references().is_empty());
    assert_eq!(paint_snapshot.implicit_name(), None);
    assert_eq!(serde_json::to_string(&paint_snapshot).unwrap(), "{}");
}
