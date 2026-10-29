use super::*;

#[test]
fn browser_authorization_on_a_narrow_screen() {
    painting_sized(
        "browser_authorization_on_a_narrow_screen",
        Some(block_app_stack::accounts::BrowserLogin {
            code: "abcd-1234-5678".into(),
            url: "https://api.example.com/auth?code=abcd-1234-5678".into(),
        }),
        None,
        false,
        vec2(390.0, 844.0),
    );
}
