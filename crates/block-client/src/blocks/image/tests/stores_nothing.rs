use super::*;

use block::Block;

#[test]
fn stores_nothing() {
    let image = Image::new();
    assert!(image.references().is_empty());
    assert_eq!(image.implicit_name(), None);
    assert_eq!(serde_json::to_string(&image).unwrap(), "{}");
}
