use super::*;

/// A redaction is told to the log's followers: a device following the log drops the payload it
/// kept, and the entry stays in the log with its leaf.
#[tokio::test]
async fn a_redaction_is_told_to_the_logs_followers() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let chat = workspace(&laptop, "Chat").await;
    laptop
        .append(
            chat,
            "test.app",
            b"take this back",
            be_block_content::Along::default(),
        )
        .await
        .unwrap();
    let phone = harness.second(&owner).await;
    let mut events = phone.connection().subscribe();
    let page = phone.read_log(chat, 0, 10).await.unwrap();
    assert_eq!(page.entries[0].bytes(), Some(&b"take this back"[..]));

    laptop.redact(chat, 0).await.unwrap();
    let redacted = loop {
        if let Ok(crate::Event::Redacted(redacted)) = events.recv().await {
            break redacted;
        }
    };
    assert_eq!((redacted.block, redacted.seq), (chat, 0));
    let entry = phone
        .redacted(&redacted)
        .await
        .unwrap()
        .expect("the phone kept the entry");
    assert_eq!(entry.entry.payload, None);
    assert_eq!(entry.opened, None);
    let read = phone.read_log(chat, 0, 10).await.unwrap();
    assert_eq!(read.entries.len(), 1);
    assert_eq!(read.entries[0].entry.payload, None);
    harness.stop().await;
}
