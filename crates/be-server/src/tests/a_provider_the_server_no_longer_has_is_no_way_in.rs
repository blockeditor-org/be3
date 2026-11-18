use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn a_provider_the_server_no_longer_has_is_no_way_in() {
    let (harness, provider) = with_provider(false).await;
    harness.member("ada@example.com").await;
    let mut ada = Browser::default();
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let (status, response) = ada
        .provider_sign_in(
            &harness,
            &provider,
            "test",
            "?intent=link",
            Grant::new("ada"),
        )
        .await;
    assert_eq!(status, 303, "{response}");
    // As if the operator removed the provider since: its identity names an issuer the server
    // no longer has, so the email is the account's last way in.
    metadata_database(&harness.directory)
        .execute(
            "UPDATE identities SET subject = 'https://gone.example ' || subject WHERE kind = 'oidc'",
            [],
        )
        .unwrap();
    let identities = page_of(&ada.get(&harness, "/auth/settings").await).identities;
    let email = identities
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
}
