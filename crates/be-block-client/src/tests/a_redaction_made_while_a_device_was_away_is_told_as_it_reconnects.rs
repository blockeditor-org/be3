use super::*;

/// A redaction pushed while a device was away reaches it as it reconnects: catching up reads
/// each log it kept from its head, which names what was redacted since, told as the push it
/// missed would have been.
#[tokio::test]
async fn a_redaction_made_while_a_device_was_away_is_told_as_it_reconnects() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let chat = workspace(&laptop, "Chat").await;
    for said in ["keep this", "take this back"] {
        laptop
            .append(
                chat,
                "test.app",
                said.as_bytes(),
                be_block_content::Along::default(),
            )
            .await
            .unwrap();
    }
    let phone_refs = refs::RefsDb::memory();
    let phone_store = MemoryStore::new();
    let phone = harness.device(&owner, &phone_refs, &phone_store).await;
    assert_eq!(phone.read_log(chat, 0, 10).await.unwrap().entries.len(), 2);
    drop(phone);

    // Away, the phone hears nothing of it.
    laptop.redact(chat, 1).await.unwrap();

    let phone = harness.device(&owner, &phone_refs, &phone_store).await;
    let mut events = phone.connection().subscribe();
    phone.catch_up_redactions().await;
    let redacted = loop {
        if let Ok(crate::Event::Redacted(redacted)) = events.recv().await {
            break redacted;
        }
    };
    assert_eq!((redacted.block, redacted.seq), (chat, 1));
    let entry = phone.redacted(&redacted).await.unwrap().unwrap();
    assert_eq!(entry.entry.payload, None);

    // Caught up, it is told nothing again.
    phone.catch_up_redactions().await;
    assert!(
        !matches!(events.try_recv(), Ok(crate::Event::Redacted(_))),
        "a redaction is told once"
    );
    harness.stop().await;
}
