use super::*;

/// What is kept for an account prints, as any `Debug` might end up in a log, without its token,
/// its authorization or its identity secret.
#[test]
fn a_debug_print_shows_no_token_or_identity_secret() {
    let mut saved = account(
        ServerLocation::Remote("https://be.example".into()),
        "a@example",
    );
    saved.authorization = Some("Basic c2VjcmV0".into());
    let printed = format!(
        "{saved:?} {:?}",
        Write::SetIdentity(saved.clone(), [0xab; 32])
    );
    assert!(printed.contains("a@example"));
    for secret in [saved.token.as_str(), "c2VjcmV0", "171, 171"] {
        assert!(
            !printed.contains(secret),
            "{secret} shows in a debug print: {printed}"
        );
    }
}
