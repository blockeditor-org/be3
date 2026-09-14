use super::*;

use block::Block;

#[test]
fn stores_nothing() {
    let workspace = WorkspaceUi::new();
    assert!(workspace.references().is_empty());
    assert_eq!(workspace.implicit_name(), None);
    assert_eq!(serde_json::to_string(&workspace).unwrap(), "{}");
}
