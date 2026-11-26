use super::*;

fn offer(step: PasskeyStep) -> Option<PasskeyOffer> {
    Some(PasskeyOffer {
        step,
        options: r#"{"challenge":"c25hcHNob3Q"}"#.into(),
    })
}

#[test]
fn the_portal_captures_passkeys() {
    // Signing in with a passkey needs no email first.
    let sign_in = Page {
        passkey: offer(PasskeyStep::SignIn),
        ..page(Screen::SignIn)
    };
    painting(sign_in, [390, 844]).snapshot("portal_login_with_passkey.browser");
    let email = IdentityRow {
        id: "email".into(),
        kind: "email".into(),
        label: "alex@example.com".into(),
        removable: true,
    };
    let passkey = IdentityRow {
        id: "passkey".into(),
        kind: "passkey".into(),
        label: "Passkey added 2026-11-15 14:02 UTC (q3Xv9A)".into(),
        removable: true,
    };
    // Fresh: the account's passkeys are listed for removal, and another can be added.
    let adding = Page {
        fresh: true,
        identities: vec![email, passkey.clone()],
        passkey: offer(PasskeyStep::Add),
        ..signed_in(Screen::Settings)
    };
    painting(adding, [1100, 1100]).snapshot("portal_settings_add_passkey.browser");
    // An account whose only way in is a passkey proves with it.
    let alone = Page {
        account: Some(Account {
            display_name: "Alex".into(),
            email: String::new(),
        }),
        email: String::new(),
        identities: vec![IdentityRow {
            removable: false,
            ..passkey
        }],
        passkey: offer(PasskeyStep::Proof),
        ..signed_in(Screen::Settings)
    };
    painting(alone, [1100, 1100]).snapshot("portal_settings_passkey_only.browser");
}
