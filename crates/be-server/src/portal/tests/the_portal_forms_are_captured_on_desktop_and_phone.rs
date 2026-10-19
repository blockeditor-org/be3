use super::*;

#[test]
fn the_portal_forms_are_captured_on_desktop_and_phone() {
    painting(page("/auth/login"), [1100, 900]).snapshot("portal_login.browser");
    painting(page("/auth/login"), [390, 844]).snapshot("portal_login_phone.browser");
    painting(page("/auth/register"), [1100, 900]).snapshot("portal_registration.browser");
    painting(page("/auth/register"), [390, 844]).snapshot("portal_registration_phone.browser");
    painting(page("/auth/forgot"), [390, 844]).snapshot("portal_forgot_password.browser");
    painting(page("/auth/reset"), [390, 844]).snapshot("portal_reset_password.browser");
    painting(page("/auth/verify"), [390, 844]).snapshot("portal_verify_email.browser");
}
