use super::*;

#[test]
fn browser_launch_failure_keeps_the_fallback_visible() {
    painting(
        "browser_launch_failure_keeps_the_fallback_visible",
        Some(block_app_stack::accounts::BrowserLogin {
            code: "abcd-1234-5678".into(),
            url: "https://api.example.com/auth?code=abcd-1234-5678".into(),
        }),
        Some("Could not open a browser. Use the link and code below.".into()),
        false,
    );
}
