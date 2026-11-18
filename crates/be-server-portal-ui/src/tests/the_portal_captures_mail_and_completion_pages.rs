use super::*;

#[test]
fn the_portal_captures_mail_and_completion_pages() {
    painting(page(Screen::CheckEmail), [390, 844]).snapshot("portal_check_email.browser");
    painting(page(Screen::LoginLink), [390, 844]).snapshot("portal_login_link.browser");
    painting(page(Screen::EmailUpdated), [390, 844]).snapshot("portal_email_updated.browser");
    painting(page(Screen::Approved), [390, 844]).snapshot("portal_approved.browser");
    dark(page(Screen::Denied), [390, 844]).snapshot("portal_denied_dark.browser");
}
