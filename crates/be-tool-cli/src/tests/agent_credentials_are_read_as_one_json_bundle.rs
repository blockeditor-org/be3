use super::*;

#[test]
fn agent_credentials_are_read_as_one_json_bundle() {
    let organization = uuid::Uuid::from_u128(42);
    let credentials = serde_json::json!({
        "server": "https://example.com",
        "organization": organization,
        "token": "agent-token",
        "recovery_phrase": "agent phrase",
    })
    .to_string();
    let settings = Settings::from_values(|name| match name {
        "BE3_AGENT_CREDENTIALS" => Some(credentials.clone()),
        "BE3_TOKEN" => Some("stale-token".into()),
        "BE3_CACHE" => Some("/tmp/cache".into()),
        "BE3_AUTHORIZATION" => Some("Bearer gateway".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(settings.server, "https://example.com");
    assert_eq!(settings.organization, Some(organization));
    assert_eq!(settings.token, "agent-token");
    assert_eq!(settings.phrase, "agent phrase");
    assert_eq!(settings.authorization.as_deref(), Some("Bearer gateway"));
    // A debug print, which may end up in a log, holds none of the secrets.
    let printed = format!("{settings:?}");
    for secret in ["agent-token", "agent phrase", "gateway"] {
        assert!(!printed.contains(secret), "{secret} shows in {printed}");
    }
}
