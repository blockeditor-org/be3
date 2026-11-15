use super::*;

#[tokio::test]
async fn a_passkey_for_an_old_origin_is_no_way_in() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let mut ada = Browser::at_localhost(&harness);
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let mut authenticator = ada.add_passkey(&harness).await;
    // As if `--auth-origin` changed since: the passkey was made for another RP ID.
    metadata_database(&harness.directory)
        .execute(
            "UPDATE identities SET subject = replace(subject, 'localhost ', 'old.example ') WHERE kind = 'passkey'",
            [],
        )
        .unwrap();
    let page = page_of(&ada.get(&harness, "/auth/settings").await);
    let email = page
        .identities
        .iter()
        .find(|identity| identity.kind == "email")
        .unwrap();
    assert!(!email.removable);
    let (status, response) = ada
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/identity/remove",
            &[("identity", &email.id)],
        )
        .await;
    assert_eq!(status, 400, "{response}");
    assert!(response.contains("only way to sign in"), "{response}");
    // Nor does it sign in, with the same answer as a passkey the server never saw.
    let mut elsewhere = Browser::at_localhost(&harness);
    let (status, response) = elsewhere
        .sign_in_with_passkey(&harness, &mut authenticator)
        .await;
    assert_eq!(status, 400, "{response}");
    assert!(response.contains("did not work here"), "{response}");
}
