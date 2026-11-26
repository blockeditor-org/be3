use super::*;
use crate::store::login_links::{self, LinkFor};

/// An intruder moved the account's email to their own; the owner asks the operator, who makes
/// a link to the account. Using it keeps no email, since the link vouched for none, so the
/// intruder's address is no way back in and hears nothing of the recovery
/// (guides/blocks/server.md, "Accounts").
#[tokio::test]
async fn a_login_link_to_an_account_keeps_no_email_it_did_not_check() {
    let harness = Harness::start().await;
    harness.member("owner@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let mut owner = Browser::default();
    owner.sign_in_by_code(&harness, "owner@example.com").await;
    let account = store.browser_profile(&owner.cookie).await.unwrap().account;
    let database = metadata_database(&harness.directory);
    database
        .execute(
            "UPDATE accounts SET email = 'intruder@example.com' WHERE id = ?1",
            [account.to_string()],
        )
        .unwrap();
    database
        .execute(
            "UPDATE identities SET subject = 'intruder@example.com' WHERE account_id = ?1",
            [account.to_string()],
        )
        .unwrap();

    let link = login_links::create(&harness.directory, LinkFor::Account(account)).unwrap();
    assert_eq!(
        link.email, "intruder@example.com",
        "the operator is shown it"
    );
    assert!(!link.vouches);
    let mut browser = Browser::default();
    let (status, response) = browser
        .submit(
            &harness,
            &login_links::path(&link.token),
            "/auth/link",
            &[("token", &link.token)],
        )
        .await;
    assert_eq!(status, 303, "{response}");
    assert!(store.identities(account).await.unwrap().is_empty());
    let profile = store.browser_profile(&browser.cookie).await.unwrap();
    assert_eq!(
        profile.email, "",
        "the intruder's address is no longer the account's"
    );
    assert_eq!(harness.mails_to("intruder@example.com"), 0);
    // A code to that address now signs nobody in.
    let mut intruder = Browser::default();
    intruder
        .submit(
            &harness,
            "/auth/login",
            "/auth/code/send",
            &[("email", "intruder@example.com"), ("purpose", "sign-in")],
        )
        .await;
    assert_eq!(harness.mails_to("intruder@example.com"), 0);
}
