use super::*;

#[test]
fn a_terminal_host_runs_only_on_the_sandbox_it_was_installed_for() {
    let credentials = serde_json::json!({
        "server": "https://example.com",
        "organization": uuid::Uuid::from_u128(42),
        "token": "agent-token",
        "recovery_phrase": "agent phrase",
        "block": uuid::Uuid::from_u128(7),
        "boat_id": "bx_r4t8w3ne",
    })
    .to_string();
    let settings = Settings::from_values(|name| match name {
        "BE3_AGENT_CREDENTIALS" => Some(credentials.clone()),
        "BE3_CACHE" => Some("/tmp/cache".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(settings.boat_id.as_deref(), Some("bx_r4t8w3ne"));
    assert!(settings.check_machine(Some("bx_r4t8w3ne")).is_ok());
    assert!(settings.check_machine(None).is_ok());
    let fork = settings.check_machine(Some("bx_f0rk2abc")).unwrap_err();
    assert!(fork.contains("bx_f0rk2abc"), "{fork}");
}
