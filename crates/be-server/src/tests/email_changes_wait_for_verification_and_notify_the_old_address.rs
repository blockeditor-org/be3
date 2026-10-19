use super::*;

#[tokio::test]
async fn email_changes_wait_for_verification_and_notify_the_old_address() {
    let harness = Harness::start().await;
    harness.member("before@example.com").await;
    let mut browser = Browser::default();
    browser
        .login(&harness, "before@example.com", "correct horse battery")
        .await;
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/settings",
                "/auth/settings/name",
                &[("display_name", "New name")]
            )
            .await
            .0,
        303
    );
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/settings",
                "/auth/settings/email",
                &[
                    ("email", "after@example.com"),
                    ("current_password", "wrong")
                ]
            )
            .await
            .0,
        400
    );
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/settings",
                "/auth/settings/email",
                &[
                    ("email", "after@example.com"),
                    ("current_password", "correct horse battery")
                ]
            )
            .await
            .0,
        303
    );
    let store = ServerStore::open(&harness.directory).unwrap();
    let profile = store.browser_profile(&browser.cookie).await.unwrap();
    assert_eq!(profile.email, "before@example.com");
    assert_eq!(profile.display_name, "New name");
    let token = harness.mail_token("after@example.com");
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/verify",
                "/auth/verify",
                &[("token", &token)]
            )
            .await
            .0,
        303
    );
    assert_eq!(
        store.browser_profile(&browser.cookie).await.unwrap().email,
        "after@example.com"
    );
    assert!(
        harness
            .mail
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|mail| mail.recipient == "before@example.com" && mail.subject.contains("changed"))
    );
}
