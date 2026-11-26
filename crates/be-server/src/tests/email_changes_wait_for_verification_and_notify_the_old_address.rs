use super::*;

#[tokio::test]
async fn email_changes_wait_for_verification_and_notify_the_old_address() {
    // Changing the email needs a fresh proof: one older than `FRESH_PROOF` is not.
    let harness = Harness::start().await;
    harness.member("before@example.com").await;
    let mut browser = Browser::default();
    browser
        .sign_in_by_code(&harness, "before@example.com")
        .await;
    harness.stale_proofs();
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
                &[("email", "after@example.com")]
            )
            .await
            .0,
        400
    );
    browser.prove(&harness, "before@example.com").await;
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/settings",
                "/auth/settings/email",
                &[("email", "after@example.com")]
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
    // A link works once.
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
        400
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
