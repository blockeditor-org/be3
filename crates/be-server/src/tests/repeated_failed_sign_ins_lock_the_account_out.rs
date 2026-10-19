use super::*;
#[tokio::test]
async fn repeated_failed_sign_ins_lock_the_account_out() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let mut browser = Browser::default();
    for _ in 0..5 {
        let (status, page) = browser
            .submit(
                &harness,
                "/auth/login",
                "/auth/login",
                &[
                    ("email", "owner@example.com"),
                    ("password", "wrong password"),
                ],
            )
            .await;
        assert_eq!(status, 400);
        assert!(!page.contains("try again"));
    }
    let (status, page) = browser
        .submit(
            &harness,
            "/auth/login",
            "/auth/login",
            &[
                ("email", "owner@example.com"),
                ("password", "correct horse battery"),
            ],
        )
        .await;
    assert_eq!(status, 400);
    assert!(page.contains("try again"));
    let (_, page) = browser
        .submit(
            &harness,
            "/auth/login",
            "/auth/login",
            &[
                ("email", "nobody@example.com"),
                ("password", "correct horse battery"),
            ],
        )
        .await;
    assert!(!page.contains("try again"));
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/forgot",
                "/auth/forgot",
                &[("email", "owner@example.com")]
            )
            .await
            .0,
        303
    );
    let reset = harness.mail_token("owner@example.com");
    assert_eq!(
        browser
            .submit(
                &harness,
                "/auth/reset",
                "/auth/reset",
                &[
                    ("token", &reset),
                    ("password", "a new password"),
                    ("confirmation", "a new password")
                ]
            )
            .await
            .0,
        303
    );
    browser
        .login(&harness, "owner@example.com", "a new password")
        .await;
    harness.stop().await;
}
