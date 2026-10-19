use super::*;

#[test]
fn the_portal_captures_mail_and_completion_pages() {
    painting(page("/auth/check-email"), [390, 844]).snapshot("portal_check_email.browser");
    painting(page("/auth/reset-requested"), [390, 844]).snapshot("portal_reset_requested.browser");
    painting(page("/auth/password-reset"), [390, 844]).snapshot("portal_password_reset.browser");
    painting(page("/auth/email-updated"), [390, 844]).snapshot("portal_email_updated.browser");
    painting(page("/auth/approved"), [390, 844]).snapshot("portal_approved.browser");
    painting(page("/auth/denied"), [390, 844]).snapshot("portal_denied.browser");
}
