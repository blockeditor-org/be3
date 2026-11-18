use super::*;

/// An email change's link alone confirms nothing: an account asking for a stranger's address
/// can't have the stranger's click attach it. Only a browser signed in as the account that
/// asked confirms it, and opening the link anywhere else leaves it unspent.
#[tokio::test]
async fn an_email_change_is_confirmed_only_in_the_account_that_asked_for_it() {
    let harness = Harness::start().await;
    harness.member("asker@example.com").await;
    harness.member("other@example.com").await;
    let mut asker = Browser::default();
    asker.sign_in_by_code(&harness, "asker@example.com").await;
    asker.prove(&harness, "asker@example.com").await;
    let (status, page) = asker
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/email",
            &[("email", "stranger@example.com")],
        )
        .await;
    assert_eq!(status, 303, "{page}");
    let token = harness.mail_token("stranger@example.com");
    let store = ServerStore::open(&harness.directory).unwrap();
    let email = |cookie: String| {
        let store = &store;
        async move { store.browser_profile(&cookie).await.unwrap().email }
    };

    // The stranger opens the link signed out, or signed in as another account.
    let mut signed_out = Browser::default();
    let (status, page) = signed_out
        .submit(
            &harness,
            "/auth/verify",
            "/auth/verify",
            &[("token", &token)],
        )
        .await;
    assert_eq!(status, 400, "{page}");
    assert!(page.contains("Sign in as the account"), "{page}");
    let mut other = Browser::default();
    other.sign_in_by_code(&harness, "other@example.com").await;
    let (status, page) = other
        .submit(
            &harness,
            "/auth/verify",
            "/auth/verify",
            &[("token", &token)],
        )
        .await;
    assert_eq!(status, 400, "{page}");
    assert_eq!(email(other.cookie.clone()).await, "other@example.com");
    assert_eq!(email(asker.cookie.clone()).await, "asker@example.com");

    // Neither spent the link: the account that asked still confirms it.
    let (status, page) = asker
        .submit(
            &harness,
            "/auth/verify",
            "/auth/verify",
            &[("token", &token)],
        )
        .await;
    assert_eq!(status, 303, "{page}");
    assert_eq!(email(asker.cookie.clone()).await, "stranger@example.com");
}
