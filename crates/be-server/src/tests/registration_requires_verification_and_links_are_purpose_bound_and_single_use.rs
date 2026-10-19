use super::*;

#[tokio::test]
async fn registration_requires_verification_and_links_are_purpose_bound_and_single_use() {
    let harness = Harness::start().await;
    let mut browser = Browser::default();
    let (status, response) = browser
        .submit(
            &harness,
            "/auth/register",
            "/auth/register",
            &[
                ("email", "new@example.com"),
                ("display_name", "New"),
                ("password", "a good password"),
                ("confirmation", "a good password"),
            ],
        )
        .await;
    assert_eq!(status, 303);
    assert!(!response.contains("token="));
    let store = ServerStore::open(&harness.directory).unwrap();
    assert!(
        store
            .login_browser("new@example.com", "a good password")
            .await
            .is_err()
    );
    let token = harness.mail_token("new@example.com");
    browser
        .get(&harness, &format!("/auth/verify?token={token}"))
        .await;
    assert!(
        store
            .login_browser("new@example.com", "a good password")
            .await
            .is_err()
    );
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/reset",
                "/auth/reset",
                &[
                    ("token", &token),
                    ("password", "another password"),
                    ("confirmation", "another password")
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
                "/auth/verify",
                "/auth/verify",
                &[("token", &token)]
            )
            .await
            .0,
        303
    );
    assert!(store.browser_profile(&browser.cookie).await.is_ok());
    assert!(store.resolve_token(&browser.cookie).await.is_err());
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
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/register",
                "/auth/register",
                &[
                    ("email", "new@example.com"),
                    ("display_name", "Other"),
                    ("password", "a good password"),
                    ("confirmation", "a good password")
                ]
            )
            .await
            .0,
        400
    );
}
