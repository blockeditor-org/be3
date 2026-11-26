use super::*;
use be_server_portal::IdentityChange;

#[test]
fn the_settings_page_lists_the_last_months_changes() {
    let page = Page {
        identities: vec![IdentityRow {
            id: "email".into(),
            kind: "email".into(),
            label: "alex@example.com".into(),
            removable: false,
        }],
        identity_changes: vec![
            IdentityChange {
                text: "Removed Passkey added 2026-11-02 09:15 UTC (q3Xv9A)".into(),
                when: "2026-11-14 18:40 UTC".into(),
            },
            IdentityChange {
                text: "Added Google (alex@example.com)".into(),
                when: "2026-11-03 10:02 UTC".into(),
            },
        ],
        ..signed_in(Screen::Settings)
    };
    painting(page, [1100, 1300]).snapshot("portal_settings_recent_changes.browser");
}
