use super::*;

#[test]
fn the_portal_captures_signing_in_with_providers() {
    let google = ProviderLink {
        name: "Google".into(),
        href: "/auth/oidc/google/start".into(),
    };
    let sign_in = Page {
        providers: vec![
            google.clone(),
            ProviderLink {
                name: "Example Corp".into(),
                href: "/auth/oidc/corp/start".into(),
            },
        ],
        ..page(Screen::SignIn)
    };
    painting(sign_in, [390, 844]).snapshot("portal_login_with_providers.browser");
    let registering = Page {
        provider: "Google".into(),
        display_name: "Alex Example".into(),
        ..page(Screen::ProviderRegister)
    };
    painting(registering, [390, 844]).snapshot("portal_provider_register.browser");
    let linking = Page {
        provider: "Google".into(),
        code_email: "alex@example.com".into(),
        ..page(Screen::EnterCode)
    };
    painting(linking, [390, 844]).snapshot("portal_link_provider_code.browser");
    // An account whose only way in is a provider: nudged to add another, and it proves with it.
    let alone = Page {
        account: Some(Account {
            display_name: "Alex".into(),
            email: String::new(),
        }),
        email: String::new(),
        identities: vec![IdentityRow {
            id: "provider".into(),
            kind: "oidc".into(),
            label: "Google".into(),
            removable: false,
        }],
        proof_providers: vec![ProviderLink {
            name: "Google".into(),
            href: "/auth/oidc/google/start?intent=proof".into(),
        }],
        ..signed_in(Screen::Settings)
    };
    painting(alone, [1100, 1100]).snapshot("portal_settings_provider_only.browser");
    // Fresh, with an email and a provider to add.
    let adding = Page {
        fresh: true,
        identities: vec![IdentityRow {
            id: "email".into(),
            kind: "email".into(),
            label: "alex@example.com".into(),
            removable: false,
        }],
        providers: vec![ProviderLink {
            name: "Google".into(),
            href: "/auth/oidc/google/start?intent=link".into(),
        }],
        ..signed_in(Screen::Settings)
    };
    painting(adding, [1100, 1100]).snapshot("portal_settings_add_provider.browser");
}
