use super::*;
use crate::store::invite_codes;

async fn register(
    harness: &Harness,
    browser: &mut Browser,
    invite: &str,
    email: &str,
) -> (u16, String) {
    browser
        .submit(
            harness,
            "/auth/register",
            "/auth/register",
            &[
                ("invite", invite),
                ("email", email),
                ("display_name", "New"),
            ],
        )
        .await
}

/// Enters the code mailed to `email` in the browser that asked for it.
async fn enter(harness: &Harness, browser: &mut Browser, email: &str) -> (u16, String) {
    let (code, _) = harness.mailed_code(email);
    browser.enter_code(harness, &code).await
}

#[tokio::test]
async fn registration_needs_an_invite_code_spent_when_the_account_is_made() {
    let harness = Harness::start().await;
    let mut browser = Browser::default();
    let (status, page) = register(&harness, &mut browser, "", "none@example.com").await;
    assert_eq!(status, 400);
    assert!(page.contains("\"field\":\"Invite\""), "{page}");
    assert!(harness.mail.0.lock().unwrap().is_empty());

    let (once, record) = invite_codes::create(&harness.directory, Some(1), Some(7)).unwrap();
    assert_eq!(record.uses, Some(1));
    assert!(record.expires.is_some());
    let typed = once.to_uppercase().replace('-', " ");
    // Asking for a code spends nothing: two people can wait on one use.
    let mut first = Browser::default();
    assert_eq!(
        register(&harness, &mut first, &typed, "first@example.com")
            .await
            .0,
        303
    );
    let mut second = Browser::default();
    assert_eq!(
        register(&harness, &mut second, &once, "second@example.com")
            .await
            .0,
        303
    );
    assert_eq!(
        enter(&harness, &mut first, "first@example.com").await.0,
        303
    );
    let (status, page) = enter(&harness, &mut second, "second@example.com").await;
    assert_eq!(status, 400);
    assert!(page.contains("\"field\":\"Invite\""), "{page}");
    assert_eq!(invite_codes::list(&harness.directory).unwrap()[0].used, 1);
    let (status, _) = register(&harness, &mut browser, &once, "third@example.com").await;
    assert_eq!(status, 400);

    let (expired, _) = invite_codes::create(&harness.directory, None, Some(0)).unwrap();
    assert_eq!(
        register(&harness, &mut browser, &expired, "late@example.com")
            .await
            .0,
        400
    );

    let (revoked, record) = invite_codes::create(&harness.directory, None, None).unwrap();
    assert!(invite_codes::revoke(&harness.directory, &record.id).unwrap());
    assert!(!invite_codes::revoke(&harness.directory, &record.id).unwrap());
    assert_eq!(
        register(&harness, &mut browser, &revoked, "revoked@example.com")
            .await
            .0,
        400
    );
    assert_eq!(invite_codes::list(&harness.directory).unwrap().len(), 2);
}
