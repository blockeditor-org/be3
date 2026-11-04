use super::*;

#[test]
fn the_portal_captures_mail_and_completion_pages() {
    painting(page(Screen::CheckEmail), [390, 844]).snapshot("portal_check_email.browser");
    painting(page(Screen::ResetRequested), [390, 844]).snapshot("portal_reset_requested.browser");
    painting(page(Screen::PasswordReset), [390, 844]).snapshot("portal_password_reset.browser");
    painting(page(Screen::EmailUpdated), [390, 844]).snapshot("portal_email_updated.browser");
    painting(page(Screen::Approved), [390, 844]).snapshot("portal_approved.browser");
    dark(page(Screen::Denied), [390, 844]).snapshot("portal_denied_dark.browser");
}
