use super::*;

#[test]
fn the_portal_captures_account_management_and_approval() {
    painting(signed_in(Screen::Account), [390, 844]).snapshot("portal_account.browser");
    painting(signed_in(Screen::Settings), [1100, 1400]).snapshot("portal_settings.browser");
    let settings = Page {
        saved: true,
        ..signed_in(Screen::Settings)
    };
    painting(settings, [390, 1600]).snapshot("portal_settings_phone.browser");
    let approval = Page {
        code: "abcd-1234-5678".into(),
        ..signed_in(Screen::Approve)
    };
    painting(approval.clone(), [390, 900]).snapshot("portal_approval.browser");
    dark(approval, [1100, 900]).snapshot("portal_approval_dark.browser");
    let sessions = Page {
        sessions: vec![
            Session {
                id: "snapshot-browser-session".into(),
                kind: SessionKind::Browser,
                age: "just now".into(),
                current: true,
            },
            Session {
                id: "snapshot-app-session".into(),
                kind: SessionKind::App,
                age: "2 days ago".into(),
                current: false,
            },
        ],
        ..signed_in(Screen::Sessions)
    };
    painting(sessions, [390, 1000]).snapshot("portal_sessions.browser");
}
