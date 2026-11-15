use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn a_provider_sign_in_with_no_account_waits_for_an_invite_code() {
    // Not authoritative for email: its verified email counts for nothing.
    let (harness, provider) = with_provider(false).await;
    let mut browser = Browser::default();
    let grant = Grant::new("ada").verified("ada@example.com");
    let (status, response) = browser
        .provider_sign_in(&harness, &provider, "test", "", grant.clone())
        .await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(location(&response), "/auth/oidc/register");
    assert!(browser.cookie.is_empty());
    assert_eq!(accounts(&harness), 0);
    let page = page_of(&browser.get(&harness, "/auth/oidc/register").await);
    assert_eq!(page.screen, be_server_portal::Screen::ProviderRegister);
    assert_eq!(page.provider, "Test provider");
    assert_eq!(page.display_name, "Ada Provider");
    let (status, _) = browser
        .submit(
            &harness,
            "/auth/oidc/register",
            "/auth/oidc/register",
            &[("invite", ""), ("display_name", "Ada")],
        )
        .await;
    assert_eq!(status, 400, "registering needs the invite code");
    let (status, response) = browser
        .submit(
            &harness,
            "/auth/oidc/register",
            "/auth/oidc/register",
            &[("invite", &harness.invite()), ("display_name", "Ada")],
        )
        .await;
    assert_eq!(status, 303, "{response}");
    let store = ServerStore::open(&harness.directory).unwrap();
    let profile = store.browser_profile(&browser.cookie).await.unwrap();
    assert_eq!(profile.display_name, "Ada");
    assert_eq!(profile.email, "", "an unverified email is never stored");
    let identities = store.identities(profile.account).await.unwrap();
    assert_eq!(identities.len(), 1);
    assert_eq!(identities[0].kind, "oidc");
    assert_eq!(identities[0].email, "");
    // The next sign-in with it signs straight in.
    let mut again = Browser::default();
    let (status, response) = again
        .provider_sign_in(&harness, &provider, "test", "", grant)
        .await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(
        store.browser_profile(&again.cookie).await.unwrap().account,
        profile.account
    );
    assert_eq!(accounts(&harness), 1);
}
