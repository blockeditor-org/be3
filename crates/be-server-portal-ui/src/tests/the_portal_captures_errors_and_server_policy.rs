use super::*;

#[test]
fn the_portal_captures_errors_and_server_policy() {
    let registration = Page {
        problem: Some(Problem::from_message("Email: enter a valid email address.")),
        email: "invalid".into(),
        display_name: "Alex".into(),
        ..page(Screen::Register)
    };
    painting(registration, [390, 1100]).snapshot("portal_registration_error.browser");
    let code = Page {
        code: "expired-code".into(),
        code_problem: "This sign-in expired. Start sign-in again in Block.".into(),
        ..page(Screen::SignIn)
    };
    painting(code, [390, 1100]).snapshot("portal_expired_code.browser");
    let expired = Page {
        problem: Some(Problem::from_message(
            "this page expired; reload it and try again",
        )),
        ..page(Screen::SignIn)
    };
    painting(expired, [390, 1000]).snapshot("portal_expired_form.browser");
    painting(page(Screen::SignInClosed), [390, 844]).snapshot("portal_sign_in_disabled.browser");
    painting(page(Screen::RegistrationClosed), [390, 844])
        .snapshot("portal_registration_disabled.browser");
    painting(page(Screen::NotFound), [390, 844]).snapshot("portal_not_found.browser");
    let message = page(Screen::Message {
        title: "Unable to continue".into(),
        text: "The verification link has expired. Request another link.".into(),
    });
    painting(message, [390, 844]).snapshot("portal_message.browser");
}
