use super::*;
#[tokio::test]
async fn browser_pages_escape_codes_and_keep_available_actions() {
    let harness = Harness::start().await;
    let (status, page) = http(&harness, "GET", "/auth?code=%22%3E%3Cscript%3E", "", None).await;
    assert_eq!(status, 200);
    assert!(page.contains("value=\"&quot;&gt;&lt;script&gt;\""));
    assert!(page.contains("/auth/login"));
    assert!(page.contains("/auth/register"));
    assert!(!page.contains("<script>"));
    for constraint in ["maxlength=", "minlength=", "required=", "pattern="] {
        assert!(!page.contains(constraint));
    }
    assert!(page.contains("novalidate=\"\""));
    assert!(page.contains("<form method=\"post\" action=\"/auth/login\""));
    assert!(page.contains("<input type=\"hidden\" name=\"csrf\" value=\""));
    assert!(page.contains("<a href=\"/auth/register?code=%22%3E%3Cscript%3E\""));
}
