use super::*;

#[test]
fn metadata_fields() {
    let dev = Metadata::parse(r#"{"id":"abc","program_id":"BlockDev","future":true}"#).unwrap();
    let deployed = Metadata::parse(r#"{"id":"abc","program_id":"Block"}"#).unwrap();
    assert!(dev.default_server.is_none());
    let configured = Metadata::parse(r#"{"id":"abc","program_id":"Block","default_server":{"name":"Block","api_url":"https://api.example.com","auth_url":"https://api.example.com/auth","gateway_required":true}}"#).unwrap();
    assert!(configured.default_server.unwrap().gateway_required);
    assert!(compatible(&dev.id, &deployed.id));
    assert_ne!(dev.program_id, deployed.program_id);
    assert!(Metadata::parse("abc").is_err());
    assert!(Metadata::parse(r#"{"id":"unknown","program_id":"Block"}"#).is_err());
    assert_eq!(
        Metadata::parse(r#"{"id":"abc","app_id":"Block"}"#)
            .unwrap()
            .program_id,
        "Block"
    );
    assert!(Metadata::parse(r#"{"id":"abc"}"#).is_err());
}
