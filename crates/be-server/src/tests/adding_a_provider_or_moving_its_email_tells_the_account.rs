use super::*;
use crate::fake_provider::Grant;

const ADDED: &str = "A way to sign in to your be3 account was added";
const MOVED: &str = "Your be3 email address changed";

#[tokio::test]
async fn adding_a_provider_or_moving_its_email_tells_the_account() {
    let (harness, provider) = with_provider(true).await;
    let store = ServerStore::open(&harness.directory).unwrap();
    // Added from the settings page: the account's email is told.
    harness.member("ada@example.com").await;
    let mut ada = Browser::default();
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let (status, response) = ada
        .provider_sign_in(
            &harness,
            &provider,
            "test",
            "?intent=link",
            Grant::new("ada-at-provider"),
        )
        .await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(harness.mailed("ada@example.com", ADDED), 1);
    // Added by entering the code a provider's sign-in mailed: that email is told.
    harness.member("bob@example.com").await;
    let mut browser = Browser::default();
    let grant = Grant::new("bob-at-provider").verified("bob@example.com");
    browser
        .provider_sign_in(&harness, &provider, "test", "", grant)
        .await;
    let (code, _) = harness.mailed_code("bob@example.com");
    assert_eq!(browser.enter_code(&harness, &code).await.0, 303);
    assert_eq!(harness.mailed("bob@example.com", ADDED), 1);
    // The provider's email changes to a free one: the account's email moves with it, and the
    // old address is told.
    let mut later = Browser::default();
    let grant = Grant::new("bob-at-provider").verified("robert@example.com");
    let (status, response) = later
        .provider_sign_in(&harness, &provider, "test", "", grant)
        .await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(
        store.browser_profile(&later.cookie).await.unwrap().email,
        "robert@example.com"
    );
    assert_eq!(harness.mailed("bob@example.com", MOVED), 1);
    assert_eq!(harness.mailed("robert@example.com", MOVED), 0);
}
