use super::*;

#[test]
fn stores_nothing() {
    let tab = WebBrowserTab::new();
    assert!(tab.references().is_empty());
    assert_eq!(tab.implicit_name(), None);
    assert_eq!(serde_json::to_string(&tab).unwrap(), "{}");
}
