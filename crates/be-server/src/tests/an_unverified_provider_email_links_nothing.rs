use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn an_unverified_provider_email_links_nothing() {
    let (harness, provider) = with_provider(true).await;
    harness.member("ada@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let mut browser = Browser::default();
    let grant = Grant {
        email: Some("ada@example.com".into()),
        email_verified: false,
        ..Grant::new("someone")
    };
    let (status, response) = browser
        .provider_sign_in(&harness, &provider, "test", "", grant)
        .await;
    // It is a stranger to the server: no code to ada, and a registration of its own.
    assert_eq!(status, 303, "{response}");
    assert_eq!(location(&response), "/auth/oidc/register");
    assert_eq!(harness.mails_to("ada@example.com"), 0);
    browser
        .submit(
            &harness,
            "/auth/oidc/register",
            "/auth/oidc/register",
            &[("invite", &harness.invite()), ("display_name", "Someone")],
        )
        .await;
    let profile = store.browser_profile(&browser.cookie).await.unwrap();
    assert_eq!(profile.email, "");
    assert_eq!(
        store.identities(profile.account).await.unwrap()[0].email,
        ""
    );
}
