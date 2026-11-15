use super::*;

#[tokio::test]
async fn an_email_with_no_account_is_answered_the_same_and_sent_nothing() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let mut known = Browser::default();
    let (known_status, known_response) = known.ask_code(&harness, "owner@example.com").await;
    let mut unknown = Browser::default();
    let (unknown_status, unknown_response) = unknown.ask_code(&harness, "nobody@example.com").await;
    assert_eq!(known_status, unknown_status);
    let location = |response: &str| {
        response
            .lines()
            .find(|line| line.starts_with("Location: "))
            .map(str::to_owned)
    };
    assert_eq!(location(&known_response), location(&unknown_response));
    assert_eq!(harness.mails_to("owner@example.com"), 1);
    assert_eq!(harness.mails_to("nobody@example.com"), 0);
    // The page that asks for the code reads the same for both.
    let known_page = page_of(&known.get(&harness, "/auth/enter").await);
    let unknown_page = page_of(&unknown.get(&harness, "/auth/enter").await);
    assert_eq!(known_page.screen, unknown_page.screen);
    assert_eq!(unknown_page.code_email, "nobody@example.com");
    // Any code entered for it is just wrong.
    assert_eq!(unknown.enter_code(&harness, "22222222").await.0, 400);
}
