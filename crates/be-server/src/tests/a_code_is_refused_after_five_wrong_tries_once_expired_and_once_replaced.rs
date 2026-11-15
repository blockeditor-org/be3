use super::*;

#[tokio::test]
async fn a_code_is_refused_after_five_wrong_tries_once_expired_and_once_replaced() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let mut browser = Browser::default();
    browser.ask_code(&harness, "owner@example.com").await;
    let (code, _) = harness.mailed_code("owner@example.com");
    let wrong = if code.starts_with('2') {
        "33333333"
    } else {
        "22222222"
    };
    for _ in 0..crate::store::codes::CODE_TRIES {
        assert_eq!(browser.enter_code(&harness, wrong).await.0, 400);
    }
    let (status, page) = browser.enter_code(&harness, &code).await;
    assert_eq!(
        status, 400,
        "a sixth try, the right code, is refused: {page}"
    );
    assert!(browser.cookie.is_empty());

    browser.ask_code(&harness, "owner@example.com").await;
    let (expired, _) = harness.mailed_code("owner@example.com");
    let database = rusqlite::Connection::open(harness.directory.join("metadata.sqlite")).unwrap();
    database
        .execute("UPDATE sign_in_codes SET expires = 1 WHERE expires > 1", [])
        .unwrap();
    assert_eq!(browser.enter_code(&harness, &expired).await.0, 400);

    browser.ask_code(&harness, "owner@example.com").await;
    let (replaced, _) = harness.mailed_code("owner@example.com");
    browser.ask_code(&harness, "owner@example.com").await;
    let (latest, _) = harness.mailed_code("owner@example.com");
    assert_eq!(browser.enter_code(&harness, &replaced).await.0, 400);
    assert_eq!(browser.enter_code(&harness, &latest).await.0, 303);
}
