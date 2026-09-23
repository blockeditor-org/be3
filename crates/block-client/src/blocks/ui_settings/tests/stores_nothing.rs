use super::*;

#[test]
fn stores_nothing() {
    let settings = UiSettings::new();
    assert!(settings.references().is_empty());
    assert_eq!(settings.implicit_name(), None);
    assert_eq!(serde_json::to_string(&settings).unwrap(), "{}");
}
