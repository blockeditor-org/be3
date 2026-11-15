use super::*;

#[tokio::test]
async fn registering_by_code_makes_the_account_once_the_code_is_entered() {
    let harness = Harness::start().await;
    let invite = harness.invite();
    let mut browser = Browser::default();
    let (status, page) = browser
        .submit(
            &harness,
            "/auth/register",
            "/auth/register",
            &[
                ("invite", &invite),
                ("display_name", "Ada"),
                ("email", "ada@example.com"),
            ],
        )
        .await;
    assert_eq!(status, 303, "{page}");
    let store = ServerStore::open(&harness.directory).unwrap();
    assert!(
        store.attempt_email(&browser.attempt).await.as_deref() == Some("ada@example.com"),
        "the attempt waits on its code"
    );
    let (code, _) = harness.mailed_code("ada@example.com");
    let (status, page) = browser.enter_code(&harness, &code).await;
    assert_eq!(status, 303, "{page}");
    let profile = store.browser_profile(&browser.cookie).await.unwrap();
    assert_eq!(profile.display_name, "Ada");
    let identities = store.identities(profile.account).await.unwrap();
    assert_eq!(identities.len(), 1);
    assert_eq!(
        (identities[0].kind.as_str(), identities[0].subject.as_str()),
        ("email", "ada@example.com")
    );
    // Its only identity can't be removed.
    browser.prove(&harness, "ada@example.com").await;
    let (status, page) = browser
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/identity/remove",
            &[("identity", &identities[0].id)],
        )
        .await;
    assert_eq!(status, 400, "{page}");
    // A second registration for the same email is refused, as the guide says.
    let mut again = Browser::default();
    let (status, _) = again
        .submit(
            &harness,
            "/auth/register",
            "/auth/register",
            &[
                ("invite", &harness.invite()),
                ("display_name", "Eve"),
                ("email", "ada@example.com"),
            ],
        )
        .await;
    assert_eq!(status, 400);
}
