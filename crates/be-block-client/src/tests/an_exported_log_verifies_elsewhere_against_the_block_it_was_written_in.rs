use super::*;

/// An export carries a block's signed log opened, each entry with its author's signature, and
/// the block file keeps it: read back by another account (a contact of the author's), which
/// imports it as a block of its own, each entry still verifies against the block it was written
/// in, and against nothing else; an entry changed in the file verifies as no one's. A redacted
/// entry is carried without what it said, which leaves nothing to verify.
#[tokio::test]
async fn an_exported_log_verifies_elsewhere_against_the_block_it_was_written_in() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let chat = workspace(&owner, "Chat").await;
    for said in ["one", "two", "gone"] {
        owner.append(chat, "test.app", said.as_bytes()).await.unwrap();
    }
    owner.redact(chat, 2).await.unwrap();
    let log = owner.export_log(chat).await.unwrap();
    assert_eq!(
        log.iter()
            .map(|entry| entry.said.as_ref().map(|said| said.bytes.clone()))
            .collect::<Vec<_>>(),
        [Some(b"one".to_vec()), Some(b"two".to_vec()), None]
    );
    let file = crate::transfer::encode(&[crate::transfer::Saved {
        id: chat,
        block_type: be_type_workspace::WorkspaceContent::CONTENT_TYPE,
        parent: None,
        metadata: be_block_content::BlockMetadata::named("Chat"),
        references: Vec::new(),
        content: None,
        log,
    }]);

    let elsewhere = harness.member(&owner, "elsewhere@example.com").await;
    let read = crate::transfer::decode(&file).unwrap();
    let saved = read[0].clone();
    assert_eq!(saved.log.len(), 3);
    assert_eq!(
        elsewhere
            .verify_exported(saved.id, "test.app", &saved.log[2])
            .await
            .unwrap(),
        None,
        "a redacted entry says nothing to verify"
    );
    let mut copy = crate::transfer::renumber(read);
    let imported = import_one(&elsewhere, copy.remove(0)).await;
    assert!(!imported.is_empty(), "the block was imported");
    for entry in &saved.log[..2] {
        assert_eq!(
            elsewhere
                .verify_exported(saved.id, "test.app", entry)
                .await
                .unwrap(),
            Some(Signer::Verified),
            "an exported entry verifies against the block it was written in"
        );
        assert_eq!(
            elsewhere
                .verify_exported(Uuid::new_v4(), "test.app", entry)
                .await
                .unwrap(),
            Some(Signer::Bad),
            "and in no other block"
        );
        assert_eq!(
            elsewhere
                .verify_exported(saved.id, "another.app", entry)
                .await
                .unwrap(),
            Some(Signer::Bad),
            "nor as another app's"
        );
    }
    let mut changed = saved.log[0].clone();
    changed.said.as_mut().unwrap().bytes = b"changed in the file".to_vec();
    assert_eq!(
        elsewhere
            .verify_exported(saved.id, "test.app", &changed)
            .await
            .unwrap(),
        Some(Signer::Bad)
    );
    harness.stop().await;
}
