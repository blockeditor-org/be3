use super::*;

#[tokio::test]
async fn a_codes_link_opened_in_another_browser_shows_the_code_and_signs_nothing_in() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let mut asking = Browser::default();
    assert_eq!(asking.ask_code(&harness, "owner@example.com").await.0, 303);
    let (code, attempt) = harness.mailed_code("owner@example.com");
    let link = format!("/auth/enter?attempt={attempt}&email_code={code}");
    let mut other = Browser::default();
    let elsewhere = page_of(&other.get(&harness, &link).await);
    assert_eq!(
        elsewhere.screen,
        be_server_portal::Screen::CodeElsewhere { code: code.clone() }
    );
    assert!(elsewhere.email_code.is_empty());
    assert!(other.cookie.is_empty());
    let here = page_of(&asking.get(&harness, &link).await);
    assert_eq!(here.screen, be_server_portal::Screen::EnterCode);
    assert_eq!(here.email_code, code);
    assert!(asking.cookie.is_empty(), "the link only fills the code in");
    assert_eq!(asking.enter_code(&harness, &here.email_code).await.0, 303);
}
