use super::*;
use crate::fake_provider::{FakeProvider, Grant};

#[tokio::test]
async fn a_code_from_another_provider_or_sent_to_its_callback_is_refused() {
    let (one, two) = (FakeProvider::start().await, FakeProvider::start().await);
    let harness = Harness::configured(ServerConfig {
        providers: vec![one.provider("one", true), two.provider("two", true)].into(),
        ..ServerConfig::OPEN
    })
    .await;
    let mut browser = Browser::default();
    // Begun with one and answered by two: one doesn't know two's code.
    let url = browser.start_provider(&harness, "one", "").await;
    let back = two.authorize(&url, Grant::new("ada"));
    assert_eq!(browser.provider_callback(&harness, &back).await.0, 400);
    // Begun with one and sent back to two's callback: the state is one's.
    let url = browser.start_provider(&harness, "one", "").await;
    let back = one
        .authorize(&url, Grant::new("ada"))
        .replace("/auth/oidc/one/", "/auth/oidc/two/");
    assert_eq!(browser.provider_callback(&harness, &back).await.0, 400);
    assert!(browser.cookie.is_empty());
    assert_eq!(accounts(&harness), 0);
}
