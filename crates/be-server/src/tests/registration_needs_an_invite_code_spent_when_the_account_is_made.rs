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
                ("password", "a good password"),
                ("confirmation", "a good password"),
            ],
        )
        .await
}

async fn verify(harness: &Harness, token: &str) -> (u16, String) {
    Browser::default()
        .submit(harness, "/auth/verify", "/auth/verify", &[("token", token)])
        .await
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
    assert_eq!(
        register(&harness, &mut browser, &typed, "first@example.com")
            .await
            .0,
        303
    );
    assert_eq!(
        register(&harness, &mut browser, &once, "second@example.com")
            .await
            .0,
        303
    );
    let first = harness.mail_token("first@example.com");
    let second = harness.mail_token("second@example.com");
    assert_eq!(verify(&harness, &first).await.0, 303);
    let (status, page) = verify(&harness, &second).await;
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
