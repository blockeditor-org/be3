use super::*;

#[tokio::test]
async fn a_change_to_the_ways_to_sign_in_is_told_on_every_device_for_a_week() {
    let harness = Harness::start().await;
    harness.member("ada@example.com").await;
    let mut ada = Browser::at_localhost(&harness);
    ada.sign_in_by_code(&harness, "ada@example.com").await;
    let store = ServerStore::open(&harness.directory).unwrap();
    let account = store.browser_profile(&ada.cookie).await.unwrap().account;
    let device = || async {
        let token = store.issue_session(account).await.unwrap();
        harness.signed_in(&token).await
    };
    let (mut first, mut second) = (device().await, device().await);
    // A passkey added: mailed, since the account has an email, and told on its devices.
    ada.add_passkey(&harness).await;
    assert_eq!(
        harness.mailed(
            "ada@example.com",
            "A way to sign in to your be3 account was added"
        ),
        1
    );
    let listed = first
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::AccountNotice>>(be_block_protocol::Query::Notices)
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(listed[0].text.contains("can now sign in"), "{listed:?}");
    // Whoever it warns about may hold a session: dismissed in its first week, it stays for
    // every other device.
    let id = listed[0].id;
    let dismissed = second
        .call(|peer| peer.write::<()>(be_block_protocol::Op::DismissNotice(id)))
        .await;
    assert!(matches!(dismissed, Ok(())), "{dismissed:?}");
    let listed = first
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::AccountNotice>>(be_block_protocol::Query::Notices)
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
    // The settings page lists it too, which no device can hide.
    let changes = page_of(&ada.get(&harness, "/auth/settings").await).identity_changes;
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert!(
        changes[0].text.starts_with("Added Passkey added "),
        "{changes:?}"
    );
    // A week on, a dismissal ends it everywhere; another account's dismissal does nothing.
    metadata_database(&harness.directory)
        .execute(
            "UPDATE account_notices SET created = created - 8 * 86400",
            [],
        )
        .unwrap();
    let mut other = harness.member("other@example.com").await;
    let _ = other
        .call(|peer| peer.write::<()>(be_block_protocol::Op::DismissNotice(id)))
        .await;
    assert_eq!(
        first
            .call(|peer| peer
                .read::<Vec<be_block_protocol::AccountNotice>>(be_block_protocol::Query::Notices))
            .await
            .unwrap()
            .len(),
        1
    );
    let dismissed = second
        .call(|peer| peer.write::<()>(be_block_protocol::Op::DismissNotice(id)))
        .await;
    assert!(matches!(dismissed, Ok(())), "{dismissed:?}");
    let listed = first
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::AccountNotice>>(be_block_protocol::Query::Notices)
        })
        .await
        .unwrap();
    assert!(listed.is_empty(), "{listed:?}");
}
