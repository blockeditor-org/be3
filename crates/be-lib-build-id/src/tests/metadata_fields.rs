use super::*;

#[test]
fn metadata_fields() {
    let dev = Metadata::parse(r#"{"id":"abc","program_id":"be-host-dev","future":true}"#).unwrap();
    let deployed = Metadata::parse(r#"{"id":"abc","program_id":"be-host"}"#).unwrap();
    assert!(dev.default_server.is_none());
    let configured = Metadata::parse(r#"{"id":"abc","program_id":"be-host","default_server":{"name":"be3","api_url":"https://api.example.com","auth_url":"https://api.example.com/auth","gateway_required":true}}"#).unwrap();
    assert!(configured.default_server.unwrap().gateway_required);
    assert!(compatible(&dev.id, &deployed.id));
    assert_ne!(dev.program_id, deployed.program_id);
    assert!(Metadata::parse("abc").is_err());
    assert!(Metadata::parse(r#"{"id":"unknown","program_id":"be-host"}"#).is_err());
    assert!(Metadata::parse(r#"{"id":"abc"}"#).is_err());
}
