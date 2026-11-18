use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn removing_the_email_it_signs_in_with_clears_the_accounts_email() {
    let (harness, provider) = with_provider(true).await;
    harness.member("ada@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let mut ada = Browser::default();
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let account = store.browser_profile(&ada.cookie).await.unwrap().account;
    let grant = Grant::new("ada-at-provider").verified("ada@example.com");
    let (status, response) = ada
        .provider_sign_in(&harness, &provider, "test", "?intent=link", grant.clone())
        .await;
    assert_eq!(status, 303, "{response}");
    let identities = store.identities(account).await.unwrap();
    let email = identities
        .iter()
        .find(|identity| identity.kind == "email")
        .unwrap();
    let (status, page) = ada
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/identity/remove",
            &[("identity", &email.id)],
        )
        .await;
    assert_eq!(status, 303, "{page}");
    assert_eq!(
        harness.mailed(
            "ada@example.com",
            "A way to sign in to your be3 account was removed"
        ),
        1
    );
    // The account has no email now, and its provider no longer gives it one.
    assert_eq!(store.browser_profile(&ada.cookie).await.unwrap().email, "");
    let identities = store.identities(account).await.unwrap();
    assert_eq!(identities.len(), 1);
    assert_eq!(identities[0].email, "");
    // Signing in with the provider again doesn't bring the removed email back.
    let mut again = Browser::default();
    let (status, response) = again
        .provider_sign_in(&harness, &provider, "test", "", grant)
        .await;
    assert_eq!(status, 303, "{response}");
    let profile = store.browser_profile(&again.cookie).await.unwrap();
    assert_eq!((profile.account, profile.email.as_str()), (account, ""));
}
