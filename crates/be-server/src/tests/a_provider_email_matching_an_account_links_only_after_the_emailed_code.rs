use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn a_provider_email_matching_an_account_links_only_after_the_emailed_code() {
    let (harness, provider) = with_provider(true).await;
    harness.member("ada@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let mut ada = Browser::default();
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let account = store.browser_profile(&ada.cookie).await.unwrap().account;
    let grant = Grant::new("ada-at-provider").verified("Ada@Example.com");
    // A browser signed in with a stale proof can't add it either, at start or by callback.
    harness.stale_proofs();
    let (status, _) = ada
        .request(&harness, "GET", "/auth/oidc/test/start?intent=link", "")
        .await;
    assert_eq!(status, 400);
    // Signing in with it makes no account and links nothing: it mails ada a code.
    let mut browser = Browser::default();
    let (status, response) = browser
        .provider_sign_in(&harness, &provider, "test", "", grant.clone())
        .await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(location(&response), "/auth/enter?linking=test");
    assert!(browser.cookie.is_empty());
    assert_eq!(accounts(&harness), 1);
    assert_eq!(store.identities(account).await.unwrap().len(), 1);
    let page = page_of(&browser.get(&harness, "/auth/enter?linking=test").await);
    assert_eq!(page.provider, "Test provider");
    // Another browser holding the code can't use it: it belongs to this one's attempt.
    let (code, _) = harness.mailed_code("ada@example.com");
    let mut stranger = Browser::default();
    assert_eq!(stranger.enter_code(&harness, &code).await.0, 400);
    // Ignoring the code and signing in with the provider again still links nothing.
    let mut again = Browser::default();
    again
        .provider_sign_in(&harness, &provider, "test", "", grant.clone())
        .await;
    assert!(again.cookie.is_empty());
    assert_eq!(store.identities(account).await.unwrap().len(), 1);
    // The code signs ada in and links the provider.
    let (status, response) = browser.enter_code(&harness, &code).await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(
        store
            .browser_profile(&browser.cookie)
            .await
            .unwrap()
            .account,
        account
    );
    let identities = store.identities(account).await.unwrap();
    assert_eq!(identities.len(), 2);
    let linked = identities
        .iter()
        .find(|identity| identity.kind == "oidc")
        .unwrap();
    assert_eq!(linked.email, "ada@example.com");
    // From now on the provider signs straight in to ada's account, and never made another.
    let mut later = Browser::default();
    let (status, _) = later
        .provider_sign_in(&harness, &provider, "test", "", grant)
        .await;
    assert_eq!(status, 303);
    assert_eq!(
        store.browser_profile(&later.cookie).await.unwrap().account,
        account
    );
    assert_eq!(accounts(&harness), 1);
}
