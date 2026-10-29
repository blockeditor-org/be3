use super::*;

#[test]
fn browser_authorization_always_offers_the_link_and_code() {
    painting(
        "browser_authorization_waits_with_a_link_and_code",
        Some(block_app_stack::accounts::BrowserLogin {
            code: "abcd-1234-5678".into(),
            url: "https://api.example.com/auth?code=abcd-1234-5678".into(),
        }),
        None,
        false,
    );
}
