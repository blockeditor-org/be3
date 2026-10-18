use super::*;

#[test]
fn metadata_fields() {
    let dev = Metadata::parse(r#"{"id":"abc","app_id":"BlockDev","future":true}"#).unwrap();
    let deployed = Metadata::parse(r#"{"id":"abc","app_id":"Block"}"#).unwrap();
    assert!(compatible(&dev.id, &deployed.id));
    assert_ne!(dev.app_id, deployed.app_id);
    assert!(Metadata::parse("abc").is_err());
    assert!(Metadata::parse(r#"{"id":"unknown","app_id":"Block"}"#).is_err());
    assert!(Metadata::parse(r#"{"id":"abc"}"#).is_err());
}
