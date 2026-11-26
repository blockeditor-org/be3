use super::*;
use crate::fake_provider::Grant;

/// Whoever else holds a begun sign-in's callback URL can't spend it: opened in another browser
/// it is refused and left for the browser that began it.
#[tokio::test]
async fn a_provider_sign_in_is_not_spent_by_its_callback_in_another_browser() {
    let (harness, provider) = with_provider(false).await;
    let mut began = Browser::default();
    let url = began.start_provider(&harness, "test", "").await;
    let back = provider.authorize(&url, Grant::new("ada"));
    let mut elsewhere = Browser::default();
    let (status, response) = elsewhere.provider_callback(&harness, &back).await;
    assert_eq!(status, 400, "{response}");
    let (status, response) = began.provider_callback(&harness, &back).await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(location(&response), "/auth/oidc/register");
}
