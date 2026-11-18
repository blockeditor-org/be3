use super::*;

#[tokio::test]
async fn an_email_change_is_told_on_the_accounts_devices_and_listed() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let mut ada = Browser::default();
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let account = store.browser_profile(&ada.cookie).await.unwrap().account;
    // Signing in just now is the fresh proof an email change needs.
    let (status, response) = ada
        .submit(
            &harness,
            "/auth/settings",
            "/auth/settings/email",
            &[("email", "ada@new.example")],
        )
        .await;
    assert_eq!(status, 303, "{response}");
    let token = harness.mail_token("ada@new.example");
    let (status, response) = ada
        .submit(
            &harness,
            "/auth/settings",
            "/auth/verify",
            &[("token", &token)],
        )
        .await;
    assert_eq!(status, 303, "{response}");
    // The old address is mailed, and the account's devices and history hear of it too.
    assert_eq!(
        harness.mailed("ada@example.com", "Your be3 email address changed"),
        1
    );
    let listed = store.mailbox(account).await.unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(
        matches!(&listed[0].letter, be_block_protocol::Letter::Notice(text)
            if text.contains("changed to ada@new.example")),
        "{listed:?}"
    );
    let changes = page_of(&ada.get(&harness, "/auth/settings").await).identity_changes;
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].text, "Email changed to ada@new.example");
}
