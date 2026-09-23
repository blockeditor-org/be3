use super::*;

#[test]
fn stores_nothing() {
    let calendar = Calendar::new();
    assert!(calendar.references().is_empty());
    assert_eq!(calendar.implicit_name(), None);
    assert_eq!(serde_json::to_string(&calendar).unwrap(), "{}");
}
