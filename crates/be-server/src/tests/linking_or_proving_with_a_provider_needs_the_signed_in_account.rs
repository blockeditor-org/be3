use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn linking_or_proving_with_a_provider_needs_the_signed_in_account() {
    let (harness, provider) = with_provider(false).await;
    harness.member("ada@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let mut ada = Browser::default();
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let account = store.browser_profile(&ada.cookie).await.unwrap().account;
    // Signed out, neither a link nor a proof begins.
    for intent in ["link", "proof"] {
        let (status, _) = Browser::default()
            .request(
                &harness,
                "GET",
                &format!("/auth/oidc/test/start?intent={intent}"),
                "",
            )
            .await;
        assert_eq!(status, 400, "{intent}");
    }
    // With a fresh proof, ada adds the provider.
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
    assert_eq!(location(&response), "/auth/settings?saved=1");
    assert_eq!(store.identities(account).await.unwrap().len(), 2);
    // A proof gone stale by the time the provider answers adds nothing.
    let url = ada.start_provider(&harness, "test", "?intent=link").await;
    harness.stale_proofs();
    let back = provider.authorize(&url, Grant::new("someone-else"));
    assert_eq!(ada.provider_callback(&harness, &back).await.0, 400);
    assert_eq!(store.identities(account).await.unwrap().len(), 2);
    // Proving with a provider account that isn't ada's proves nothing; with ada's it does.
    let (status, _) = ada
        .provider_sign_in(
            &harness,
            &provider,
            "test",
            "?intent=proof",
            Grant::new("mallory"),
        )
        .await;
    assert_eq!(status, 400);
    assert!(!store.is_fresh(&ada.cookie).await.unwrap());
    let (status, response) = ada
        .provider_sign_in(
            &harness,
            &provider,
            "test",
            "?intent=proof",
            Grant::new("ada"),
        )
        .await;
    assert_eq!(status, 303, "{response}");
    assert!(store.is_fresh(&ada.cookie).await.unwrap());
    // The provider account that signs in to ada's can't be added to another account.
    harness.member("eve@example.com").await;
    let mut eve = Browser::default();
    eve.sign_in_by_code(&harness, "eve@example.com").await;
    let (status, _) = eve
        .provider_sign_in(
            &harness,
            &provider,
            "test",
            "?intent=link",
            Grant::new("ada"),
        )
        .await;
    assert_eq!(status, 400);
    // A link ada began, answered once the browser is signed in as someone else, links nothing.
    let url = ada.start_provider(&harness, "test", "?intent=link").await;
    let own = std::mem::replace(&mut ada.cookie, eve.cookie.clone());
    let back = provider.authorize(&url, Grant::new("carol"));
    assert_eq!(ada.provider_callback(&harness, &back).await.0, 400);
    ada.cookie = own;
    assert_eq!(store.identities(account).await.unwrap().len(), 2);
    let eve_account = store.browser_profile(&eve.cookie).await.unwrap().account;
    assert_eq!(store.identities(eve_account).await.unwrap().len(), 1);
}
