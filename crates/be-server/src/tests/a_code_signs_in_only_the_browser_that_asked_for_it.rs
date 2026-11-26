use super::*;

#[tokio::test]
async fn a_code_signs_in_only_the_browser_that_asked_for_it() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let mut asking = Browser::default();
    assert_eq!(asking.ask_code(&harness, "owner@example.com").await.0, 303);
    assert!(!asking.attempt.is_empty());
    let (code, _) = harness.mailed_code("owner@example.com");
    // A browser that didn't ask has no attempt the code belongs to, so the code does nothing
    // there, and still works where it was asked for.
    let mut other = Browser::default();
    let (status, page) = other.enter_code(&harness, &code).await;
    assert_eq!(status, 400, "{page}");
    assert!(other.cookie.is_empty());
    let mut stranger = Browser {
        attempt: be3_attempt_of_another_request(&harness).await,
        ..Browser::default()
    };
    assert_eq!(stranger.enter_code(&harness, &code).await.0, 400);
    assert!(stranger.cookie.is_empty());
    let (status, page) = asking.enter_code(&harness, &code).await;
    assert_eq!(status, 303, "{page}");
    let store = ServerStore::open(&harness.directory).unwrap();
    let profile = store.browser_profile(&asking.cookie).await.unwrap();
    assert_eq!(profile.email, "owner@example.com");
    // A code works once.
    assert_eq!(asking.enter_code(&harness, &code).await.0, 400);
}

/// The attempt of a browser that asked for a code of its own, for another email.
async fn be3_attempt_of_another_request(harness: &Harness) -> String {
    let mut browser = Browser::default();
    assert_eq!(browser.ask_code(harness, "nobody@example.com").await.0, 303);
    browser.attempt
}
