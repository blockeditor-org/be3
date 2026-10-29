use super::*;
#[tokio::test]
async fn browser_pages_escape_codes_and_keep_available_actions() {
    let harness = Harness::start().await;
    let (status, response) = http(&harness, "GET", "/auth?code=%22%3E%3Cscript%3E", "", None).await;
    assert_eq!(status, 200);
    assert!(!response.contains("<script>"));
    let page = page_of(&response);
    assert_eq!(page.screen, be_portal::Screen::SignIn);
    assert_eq!(page.code, "\"><script>");
    assert!(page.allow_registration);
    assert!(!page.csrf.is_empty());
}
