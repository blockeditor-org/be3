use super::*;

use block::Block;

#[test]
fn stores_nothing() {
    let checklist = Checklist::new();
    assert!(checklist.references().is_empty());
    assert_eq!(checklist.implicit_name(), None);
    assert_eq!(serde_json::to_string(&checklist).unwrap(), "{}");
}
