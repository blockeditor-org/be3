use super::*;

#[tokio::test]
async fn browser_pages_escape_codes_and_keep_available_actions() {
    let harness = Harness::start().await;
    let (status, page) = http(&harness, "GET", "/auth?code=%22%3E%3Cscript%3E", "", None).await;
    assert_eq!(status, 200);
    assert!(page.contains("name=\"code\" value=\"&quot;&gt;&lt;script&gt;\""));
    assert!(page.contains("value=\"login\""));
    assert!(page.contains("value=\"register\""));
    assert!(page.contains("value=\"deny\" formnovalidate=\"\""));
    assert!(!page.contains("<script>"));
}
