use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn an_old_nonce_a_reused_state_or_another_browser_is_refused() {
    let (harness, provider) = with_provider(true).await;
    // A token for an earlier request's nonce doesn't answer a later one.
    let mut browser = Browser::default();
    let first = browser.start_provider(&harness, "test", "").await;
    let old = url::Url::parse(&first)
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "nonce")
        .unwrap()
        .1
        .into_owned();
    let second = browser.start_provider(&harness, "test", "").await;
    let back = provider.authorize(
        &second,
        Grant {
            nonce: Some(old),
            ..Grant::new("ada")
        },
    );
    let (status, response) = browser.provider_callback(&harness, &back).await;
    assert_eq!(status, 400, "{response}");
    assert!(browser.cookie.is_empty());
    // A provider's answer works only in the browser that began the sign-in.
    let url = browser.start_provider(&harness, "test", "").await;
    let back = provider.authorize(&url, Grant::new("ada"));
    let mut other = Browser::default();
    let (status, _) = other.provider_callback(&harness, &back).await;
    assert_eq!(status, 400);
    assert!(other.cookie.is_empty());
    // And once: the same state again is refused, even where it began.
    let url = browser.start_provider(&harness, "test", "").await;
    let back = provider.authorize(&url, Grant::new("ada"));
    let (status, response) = browser.provider_callback(&harness, &back).await;
    assert_eq!(status, 303, "{response}");
    assert!(location(&response).starts_with("/auth/oidc/register"));
    let (status, _) = browser.provider_callback(&harness, &back).await;
    assert_eq!(status, 400);
}
