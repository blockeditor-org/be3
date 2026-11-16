use super::*;

#[test]
fn the_portal_captures_account_management_and_approval() {
    painting(signed_in(Screen::Account), [390, 844]).snapshot("portal_account.browser");
    let identities = vec![IdentityRow {
        id: "snapshot-identity".into(),
        kind: "email".into(),
        label: "alex@example.com".into(),
        removable: false,
    }];
    let settings = Page {
        identities: identities.clone(),
        ..signed_in(Screen::Settings)
    };
    painting(settings, [1100, 1600]).snapshot("portal_settings.browser");
    let settings = Page {
        saved: true,
        fresh: true,
        identities,
        ..signed_in(Screen::Settings)
    };
    painting(settings, [390, 1600]).snapshot("portal_settings_phone.browser");
    let approval = Page {
        code: "abcd-1234-5678".into(),
        fresh: true,
        ..signed_in(Screen::Approve)
    };
    painting(approval.clone(), [390, 900]).snapshot("portal_approval.browser");
    dark(approval.clone(), [1100, 900]).snapshot("portal_approval_dark.browser");
    // Signed in a while ago: sign in again before approving.
    let stale = Page {
        fresh: false,
        ..approval
    };
    painting(stale, [390, 900]).snapshot("portal_approval_stale.browser");
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
                kind: SessionKind::Host,
                age: "2 days ago".into(),
                current: false,
            },
        ],
        ..signed_in(Screen::Sessions)
    };
    painting(sessions, [390, 1000]).snapshot("portal_sessions.browser");
}
