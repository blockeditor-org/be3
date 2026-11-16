use super::*;
use crate::store::login_links::{self, LinkFor};

#[tokio::test]
async fn a_login_link_signs_in_once_and_ends_other_sessions() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let mut before = Browser::default();
    before.sign_in_by_code(&harness, "owner@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let account = store.browser_profile(&before.cookie).await.unwrap().account;
    assert!(login_links::create(&harness.directory, LinkFor::Account(Uuid::new_v4())).is_err());
    let link = login_links::create(&harness.directory, LinkFor::Account(account)).unwrap();
    assert!(!link.registers);
    // A link by the account's email finds it (made now: a link to the account, used below,
    // leaves it no email).
    let expired =
        login_links::create(&harness.directory, LinkFor::Email("Owner@Example.com")).unwrap();
    assert_eq!(expired.email, "owner@example.com");
    assert!(!expired.registers);
    let page = login_links::path(&link.token);
    let mut browser = Browser::default();
    let (status, response) = browser
        .submit(&harness, &page, "/auth/link", &[("token", &link.token)])
        .await;
    assert_eq!(status, 303, "{response}");
    assert_eq!(
        store
            .browser_profile(&browser.cookie)
            .await
            .unwrap()
            .account,
        account
    );
    // It is a fresh proof, and it ended the account's other sessions, as recovering does.
    assert!(store.is_fresh(&browser.cookie).await.unwrap());
    assert!(store.browser_profile(&before.cookie).await.is_err());
    // It works once.
    let mut again = Browser::default();
    let (status, _) = again
        .submit(&harness, &page, "/auth/link", &[("token", &link.token)])
        .await;
    assert_eq!(status, 400);
    assert!(again.cookie.is_empty());
    // An expired one is refused.
    metadata_database(&harness.directory)
        .execute("UPDATE login_links SET expires = 0", [])
        .unwrap();
    let (status, _) = again
        .submit(
            &harness,
            &login_links::path(&expired.token),
            "/auth/link",
            &[("token", &expired.token)],
        )
        .await;
    assert_eq!(status, 400);
    assert!(again.cookie.is_empty());
}
