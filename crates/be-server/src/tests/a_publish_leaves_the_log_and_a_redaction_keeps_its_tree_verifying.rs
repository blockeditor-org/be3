use super::*;

/// No commit touches a block's log: a publish leaves its head as it was. A redaction drops an
/// entry's payload and keeps everything its leaf hashes, so the log's head and every proof stay
/// as they were, and the entry still shows who wrote it; what it held is no longer charged.
#[tokio::test]
async fn a_publish_leaves_the_log_and_a_redaction_keeps_its_tree_verifying() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_id = owner.id();
    owner.personal().await;
    let chat = owner.create_block(BlockParent::Root).await;
    let logged = owner
        .append(
            chat,
            vec![
                log_entry(b"one"),
                log_entry(&[7; 4096]),
                log_entry(b"three"),
            ],
        )
        .await
        .unwrap();
    assert_eq!(logged.len(), 3);
    let before = owner.log_page(chat, 0, None, false).await.unwrap();
    assert!(verifies(&before, 0));

    let author = Author::new(owner_id);
    let (head, chunks) = author.compose(b"the chat's settings\n", 1_000, None);
    let published = owner.publish(&author, chat, head, chunks, None, 1_000).await;
    assert!(published.is_ok(), "{published:?}");
    let after = owner.log_page(chat, 0, None, false).await.unwrap();
    assert_eq!(after, before, "a publish changed the log");
    let held = owner.storage().await.unwrap().held;

    let redacted = owner.call(|peer| peer.write::<()>(Op::Redact { block: chat, seq: 1 })).await;
    assert!(redacted.is_ok(), "{redacted:?}");
    let page = owner.log_page(chat, 0, Some(3), false).await.unwrap();
    assert_eq!(page.head, before.head, "a redaction moved the head");
    assert!(verifies(&page, 0), "a redacted log no longer verifies");
    assert_eq!(page.entries[1].payload, None);
    assert_eq!(page.entries[1].author, owner_id);
    assert_eq!(page.entries[1].payload_hash, before.entries[1].payload_hash);
    assert_eq!(page.entries[0].payload.as_deref(), Some(&b"one"[..]));
    assert!(
        owner.storage().await.unwrap().held == held - 4096,
        "a redacted payload is still charged"
    );
    // Redacting it again is no refusal.
    let again = owner.call(|peer| peer.write::<()>(Op::Redact { block: chat, seq: 1 })).await;
    assert!(again.is_ok(), "{again:?}");
    let missing = owner.call(|peer| peer.write::<()>(Op::Redact { block: chat, seq: 3 })).await;
    assert!(
        matches!(&missing, Err(error) if error.code == ErrorCode::InvalidRequest),
        "{missing:?}"
    );
    harness.stop().await;
}
