use super::*;
use crate::store::codes::{CodePurpose, CodeRequest};

#[tokio::test]
async fn a_closed_server_also_refuses_browser_registration_and_login() {
    let harness = Harness::configured(ServerConfig::default()).await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let (profile, _) = store.register("ada@example.com", "Ada").await.unwrap();
    let mut browser = Browser::default();
    for (action, purpose) in [
        ("code/send", "sign-in"),
        ("code/send", "recover"),
        ("register", ""),
        ("approve", ""),
    ] {
        let (status, page) = browser
            .submit(
                &harness,
                "/auth",
                &format!("/auth/{action}"),
                &[
                    ("email", "ada@example.com"),
                    ("display_name", "Ada"),
                    ("invite", &harness.invite()),
                    ("purpose", purpose),
                ],
            )
            .await;
        assert_eq!(status, 400, "{action} {purpose}: {page}");
    }
    assert!(harness.mail.0.lock().unwrap().is_empty());
    // A right code, asked for while the server was open, signs nothing in once it is closed.
    let attempt = crate::new_secret();
    let mailed = store
        .request_code(CodeRequest {
            purpose: CodePurpose::SignIn,
            email: "ada@example.com",
            account: None,
            display_name: "",
            invite: "",
            device_code: "",
            attempt: &attempt,
            address: "127.0.0.1",
        })
        .await
        .unwrap()
        .unwrap();
    browser.attempt = attempt;
    let (status, page) = browser.enter_code(&harness, &mailed.code).await;
    assert_eq!(status, 400, "{page}");
    assert!(page.contains("not accepting sign-ins"), "{page}");
    assert!(browser.cookie.is_empty());
    // `--allow-login` gates signing in, not a session that already has: it still reaches its
    // settings.
    let mut signed_in = Browser {
        cookie: store.browser_session(profile.account).await.unwrap(),
        ..Browser::default()
    };
    let page = signed_in.get(&harness, "/auth/settings").await;
    assert_eq!(page_of(&page).screen, be_server_portal::Screen::Settings);
}
