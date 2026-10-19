use super::*;

#[test]
fn the_portal_captures_errors_and_server_policy() {
    let mut registration = page("/auth/register");
    registration.error = "Email: enter a valid email address.".into();
    registration.fields = HashMap::from([
        ("email".into(), "invalid".into()),
        ("display_name".into(), "Alex".into()),
    ]);
    painting(registration, [390, 1100]).snapshot("portal_registration_error.browser");
    let mut code = page("/auth");
    code.code = "expired-code".into();
    code.code_error = "This sign-in code expired. Start sign-in again in Block.".into();
    painting(code, [390, 1000]).snapshot("portal_expired_code.browser");
    let mut closed = page("/auth");
    closed.allow_login = false;
    painting(closed, [390, 844]).snapshot("portal_sign_in_disabled.browser");
    let mut closed = page("/auth/register");
    closed.allow_registration = false;
    painting(closed, [390, 844]).snapshot("portal_registration_disabled.browser");
    painting(page("/auth/missing"), [390, 844]).snapshot("portal_not_found.browser");
    BrowserSnapshot::new(
        [390, 844],
        &message(
            "Unable to continue",
            "The verification link has expired. Request another link.",
        ),
    )
    .snapshot("portal_message.browser");
}
