use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn a_link_or_a_proof_needs_a_sign_in_at_the_provider_just_now() {
    let (harness, provider) = with_provider(false).await;
    harness.member("ada@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let mut ada = Browser::default();
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let account = store.browser_profile(&ada.cookie).await.unwrap().account;
    // Signing in asks for no fresh sign-in at the provider; a link and a proof do.
    let url = Browser::default()
        .start_provider(&harness, "test", "")
        .await;
    assert!(!url.contains("max_age"), "{url}");
    let an_hour_ago = Grant {
        signed_in_ago: 3600,
        ..Grant::new("ada")
    };
    let url = ada.start_provider(&harness, "test", "?intent=link").await;
    assert!(url.contains("max_age=0"), "{url}");
    let back = provider.authorize(&url, an_hour_ago.clone());
    assert_eq!(ada.provider_callback(&harness, &back).await.0, 400);
    assert_eq!(store.identities(account).await.unwrap().len(), 1);
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
    assert_eq!(store.identities(account).await.unwrap().len(), 2);
    harness.stale_proofs();
    let url = ada.start_provider(&harness, "test", "?intent=proof").await;
    assert!(url.contains("max_age=0"), "{url}");
    let back = provider.authorize(&url, an_hour_ago);
    assert_eq!(ada.provider_callback(&harness, &back).await.0, 400);
    assert!(!store.is_fresh(&ada.cookie).await.unwrap());
}
