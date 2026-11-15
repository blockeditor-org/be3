use crate::oidc::{GOOGLE_ISSUER, parse_providers};

#[test]
fn the_providers_file_is_checked_and_keeps_its_secrets() {
    let providers = parse_providers(
        r#"[
            {"id": "google", "preset": "google", "client_id": "g", "client_secret": "hidden"},
            {"id": "corp", "name": "Corp", "issuer": "https://login.example.com/tenant/v2.0",
             "client_id": "c", "client_secret": "hidden"}
        ]"#,
    )
    .unwrap();
    assert_eq!(providers[0].issuer, GOOGLE_ISSUER);
    assert!(providers[0].authoritative_email);
    assert!(
        !providers[1].authoritative_email,
        "nobody is trusted for email unless marked"
    );
    assert!(!format!("{providers:?}").contains("hidden"));
    let refused = |json: &str| parse_providers(json).unwrap_err();
    let entry = |id: &str, issuer: &str| {
        format!(
            r#"{{"id": "{id}", "name": "N", "issuer": "{issuer}", "client_id": "c", "client_secret": "s"}}"#
        )
    };
    for json in [
        format!("[{}, {}]", entry("a", "https://one.example"), entry("a", "https://two.example")),
        format!("[{}, {}]", entry("a", "https://one.example"), entry("b", "https://one.example")),
        format!("[{}]", entry("a", "http://one.example")),
        format!("[{}]", entry("a", "https://login.example.com/{tenantid}/v2.0")),
        format!("[{}]", entry("a", "https://login.microsoftonline.com/common/v2.0")),
        format!("[{}]", entry("a", "https://login.microsoftonline.com/Organizations/v2.0")),
        format!("[{}]", entry("Has Space", "https://one.example")),
        r#"[{"id": "g", "preset": "google", "issuer": "https://evil.example", "client_id": "c", "client_secret": "s"}]"#.to_owned(),
    ] {
        let error = refused(&json);
        assert!(!error.contains("\"s\""), "{error}");
    }
    assert!(parse_providers(&format!("[{}]", entry("a", "http://127.0.0.1:9000"))).is_ok());
}
