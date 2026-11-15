use std::sync::atomic::Ordering;

use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn a_provider_is_called_only_so_often_whatever_strangers_send() {
    // A provider whose discovery fails is left alone for a while.
    let (harness, provider) = with_provider(false).await;
    provider.discovery_fails.store(true, Ordering::SeqCst);
    for _ in 0..3 {
        let (status, _) = Browser::default()
            .request(&harness, "GET", "/auth/oidc/test/start", "")
            .await;
        assert_eq!(status, 400);
    }
    assert_eq!(provider.discoveries.load(Ordering::SeqCst), 1);
    // A token signed by a key the provider doesn't publish reads its keys again at most once a
    // minute.
    let (harness, provider) = with_provider(false).await;
    let mut browser = Browser::default();
    assert_eq!(
        browser
            .provider_sign_in(&harness, &provider, "test", "", Grant::new("ada"))
            .await
            .0,
        303
    );
    for _ in 0..3 {
        let stranger = Grant {
            stranger_signs: true,
            ..Grant::new("ada")
        };
        let (status, _) = Browser::default()
            .provider_sign_in(&harness, &provider, "test", "", stranger)
            .await;
        assert_eq!(status, 400);
    }
    assert_eq!(provider.discoveries.load(Ordering::SeqCst), 1);
    // One address may have only so many sign-ins waiting on a provider.
    for _ in 0..crate::store::providers::PROVIDER_SIGN_INS_PER_ADDRESS {
        Browser::default()
            .start_provider(&harness, "test", "")
            .await;
    }
    let (status, response) = Browser::default()
        .request(&harness, "GET", "/auth/oidc/test/start", "")
        .await;
    assert_eq!(status, 400, "{response}");
}
