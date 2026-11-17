use super::*;

fn notices(mailbox: Vec<be_block_protocol::Message>) -> Vec<(u64, String)> {
    mailbox
        .into_iter()
        .filter_map(|message| match (message.from, message.letter) {
            (None, be_block_protocol::Letter::Notice(text)) => Some((message.id, text)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_notice_under_a_week_old_refuses_deletion_and_one_older_deletes() {
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
    // A passkey added: mailed, since the account has an email, and left in its mailbox.
    ada.add_passkey(&harness).await;
    assert_eq!(
        harness.mailed(
            "ada@example.com",
            "A way to sign in to your be3 account was added"
        ),
        1
    );
    let read = |peer: &At| {
        peer.read::<Vec<be_block_protocol::Message>>(be_block_protocol::Query::Mailbox)
    };
    let listed = notices(first.call(read).await.unwrap());
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(listed[0].1.contains("can now sign in"), "{listed:?}");
    // Whoever it warns about may hold a session: in its first week its deletion is refused, and
    // it stays for every device.
    let id = listed[0].0;
    let deleted = second
        .call(|peer| peer.write::<()>(be_block_protocol::Op::DeleteMessage(id)))
        .await;
    assert!(
        matches!(&deleted, Err(error) if error.code == ErrorCode::InvalidRequest),
        "{deleted:?}"
    );
    assert_eq!(notices(first.call(read).await.unwrap()).len(), 1);
    // The settings page lists it too, which no device can hide.
    let changes = page_of(&ada.get(&harness, "/auth/settings").await).identity_changes;
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert!(
        changes[0].text.starts_with("Added Passkey added "),
        "{changes:?}"
    );
    // A week on, a deletion ends it everywhere; another account's deletes nothing.
    metadata_database(&harness.directory)
        .execute("UPDATE mailbox SET created = created - 8 * 86_400_000", [])
        .unwrap();
    let mut other = harness.member("other@example.com").await;
    let _ = other
        .call(|peer| peer.write::<()>(be_block_protocol::Op::DeleteMessage(id)))
        .await;
    assert_eq!(notices(first.call(read).await.unwrap()).len(), 1);
    let deleted = second
        .call(|peer| peer.write::<()>(be_block_protocol::Op::DeleteMessage(id)))
        .await;
    assert!(matches!(deleted, Ok(())), "{deleted:?}");
    let listed = notices(first.call(read).await.unwrap());
    assert!(listed.is_empty(), "{listed:?}");
}
