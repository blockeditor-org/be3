use super::*;

#[test]
fn the_portal_captures_account_management_and_approval() {
    painting(signed_in("/auth"), [390, 844]).snapshot("portal_account.browser");
    painting(signed_in("/auth/settings"), [1100, 1400]).snapshot("portal_settings.browser");
    let mut settings = signed_in("/auth/settings");
    settings.saved = true;
    painting(settings, [390, 1600]).snapshot("portal_settings_phone.browser");
    let mut approval = signed_in("/auth");
    approval.code = "abcd-1234-5678".into();
    painting(approval, [390, 1000]).snapshot("portal_approval.browser");
    let mut sessions = signed_in("/auth/sessions");
    sessions.sessions = vec![
        AccountSession {
            id: "snapshot-browser-session".into(),
            kind: "browser".into(),
            created: i64::MAX,
            current: true,
        },
        AccountSession {
            id: "snapshot-app-session".into(),
            kind: "app".into(),
            created: i64::MAX,
            current: false,
        },
    ];
    painting(sessions, [390, 1100]).snapshot("portal_sessions.browser");
}
