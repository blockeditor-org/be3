use super::*;

#[test]
fn proxy_credentials_do_not_need_an_account_token() {
    let settings = Settings::from_values(|name| match name {
        "BE3_AGENT_CREDENTIALS" => Some(serde_json::json!({"server":"https://example.com","organization":uuid::Uuid::from_u128(42),"authentication":"proxy","recovery_phrase":"agent phrase"}).to_string()),
        "BE3_TOKEN" => Some("stale token".into()),
        "BE3_CACHE" => Some("/tmp/cache".into()),
        _ => None,
    }).unwrap();
    assert!(settings.token.is_empty());
}
