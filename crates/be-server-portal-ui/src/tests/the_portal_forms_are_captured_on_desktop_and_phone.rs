use super::*;

#[test]
fn the_portal_forms_are_captured_on_desktop_and_phone() {
    painting(page(Screen::SignIn), [1100, 900]).snapshot("portal_login.browser");
    painting(page(Screen::SignIn), [390, 844]).snapshot("portal_login_phone.browser");
    dark(page(Screen::SignIn), [1100, 900]).snapshot("portal_login_dark.browser");
    let approving = Page {
        code: "abcd-1234-5678".into(),
        ..page(Screen::SignIn)
    };
    painting(approving, [390, 844]).snapshot("portal_login_with_code.browser");
    painting(page(Screen::Register), [1100, 1000]).snapshot("portal_registration.browser");
    painting(page(Screen::Register), [390, 1000]).snapshot("portal_registration_phone.browser");
    painting(page(Screen::Forgot), [390, 844]).snapshot("portal_forgot_password.browser");
    painting(page(Screen::Reset), [390, 844]).snapshot("portal_reset_password.browser");
    painting(page(Screen::Verify), [390, 844]).snapshot("portal_verify_email.browser");
}
