use crate::session::Settings;

#[test]
fn malformed_agent_credentials_do_not_disclose_secrets() {
    for credentials in [
        "private-token not json",
        r#"{"token":"private-token"}"#,
        r#"{"server":42,"organization":"private-token","token":"private-token","recovery_phrase":"private-phrase"}"#,
        "[]",
    ] {
        let error = Settings::from_values(|name| {
            (name == "BE3_AGENT_CREDENTIALS").then(|| credentials.to_owned())
        })
        .unwrap_err();
        assert!(error.contains("BE3_AGENT_CREDENTIALS"));
        assert!(!error.contains("private-token"));
        assert!(!error.contains("private-phrase"));
    }
}
