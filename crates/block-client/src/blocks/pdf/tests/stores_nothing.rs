use super::*;

use block::Block;

#[test]
fn stores_nothing() {
    let pdf = Pdf::new();
    assert!(pdf.references().is_empty());
    assert_eq!(pdf.implicit_name(), None);
    assert_eq!(serde_json::to_string(&pdf).unwrap(), "{}");
}
