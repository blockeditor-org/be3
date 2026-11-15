use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn a_changed_provider_email_held_by_another_account_is_not_taken() {
    let (harness, provider) = with_provider(true).await;
    harness.member("taken@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    // An account made through the provider takes its verified email.
    let mut browser = Browser::default();
    browser
        .provider_sign_in(
            &harness,
            &provider,
            "test",
            "",
            Grant::new("ada").verified("first@example.com"),
        )
        .await;
    browser
        .submit(
            &harness,
            "/auth/oidc/register",
            "/auth/oidc/register",
            &[("invite", &harness.invite()), ("display_name", "Ada")],
        )
        .await;
    let profile = store.browser_profile(&browser.cookie).await.unwrap();
    assert_eq!(profile.email, "first@example.com");
    let sign_in = |email: &'static str| {
        let (harness, provider) = (&harness, &provider);
        async move {
            let mut browser = Browser::default();
            let (status, response) = browser
                .provider_sign_in(
                    harness,
                    provider,
                    "test",
                    "",
                    Grant::new("ada").verified(email),
                )
                .await;
            assert_eq!(status, 303, "{response}");
            browser
        }
    };
    // The provider now says another account's email: the account keeps its own.
    let changed = sign_in("taken@example.com").await;
    let profile = store.browser_profile(&changed.cookie).await.unwrap();
    assert_eq!(profile.email, "first@example.com");
    assert_eq!(accounts(&harness), 2);
    // A free one it follows, its email identity with it.
    let changed = sign_in("second@example.com").await;
    let profile = store.browser_profile(&changed.cookie).await.unwrap();
    assert_eq!(profile.email, "second@example.com");
    let emails: Vec<String> = store
        .identities(profile.account)
        .await
        .unwrap()
        .into_iter()
        .filter(|identity| identity.kind == "email")
        .map(|identity| identity.subject)
        .collect();
    assert_eq!(emails, ["second@example.com"]);
}
