use super::*;

/// An export carries a block's signed log opened, each entry with its author's signature, and
/// the block file keeps it: read back by another account (a contact of the author's), which
/// imports it as a block of its own, each entry still verifies against the block it was written
/// in, and against nothing else; an entry changed in the file verifies as no one's. A redacted
/// entry is carried without what it said, which leaves nothing to verify. Imported, the log is
/// history in the new block, each entry still as it was written, and new entries follow it;
/// an exported entry replayed after them is no one's.
#[tokio::test]
async fn an_exported_log_verifies_elsewhere_against_the_block_it_was_written_in() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let chat = workspace(&owner, "Chat").await;
    for said in ["one", "two", "gone"] {
        owner
            .append(
                chat,
                "test.app",
                said.as_bytes(),
                be_block_content::Along::default(),
            )
            .await
            .unwrap();
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
            .verify_exported("test.app", &saved.log[2])
            .await
            .unwrap(),
        None,
        "a redacted entry says nothing to verify"
    );
    let mut copy = crate::transfer::renumber(read);
    let new = copy[0].id;
    let imported = import_one(&elsewhere, copy.remove(0)).await;
    assert!(!imported.is_empty(), "the block was imported");
    // The imported block's log holds the history as it was written in the original, after
    // which its own entries go.
    elsewhere.drain(|_| false).await.unwrap();
    elsewhere
        .append(new, "test.app", b"after the import", be_block_content::Along::default())
        .await
        .unwrap();
    let history = elsewhere.read_log(new, 0, 10).await.unwrap();
    assert_eq!(
        history
            .entries
            .iter()
            .map(|logged| (
                logged.imported.as_ref().map(|original| original.block),
                logged.bytes().map(<[u8]>::to_vec)
            ))
            .collect::<Vec<_>>(),
        [
            (Some(chat), Some(b"one".to_vec())),
            (Some(chat), Some(b"two".to_vec())),
            (Some(chat), None),
            (None, Some(b"after the import".to_vec())),
        ]
    );
    assert_eq!(
        history.entries[0].imported.as_ref().map(|original| original.author),
        Some(owner.account()),
        "an imported entry keeps its original author"
    );
    for entry in &saved.log[..2] {
        assert_eq!(
            elsewhere
                .verify_exported("test.app", entry)
                .await
                .unwrap(),
            Some(Signer::Verified),
            "an exported entry verifies against the block it was written in"
        );
        assert_eq!(
            elsewhere
                .verify_exported(
                    "test.app",
                    &crate::ExportedEntry {
                        block: Uuid::new_v4(),
                        ..entry.clone()
                    },
                )
                .await
                .unwrap(),
            Some(Signer::Bad),
            "and in no other block"
        );
        assert_eq!(
            elsewhere
                .verify_exported("another.app", entry)
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
            .verify_exported("test.app", &changed)
            .await
            .unwrap(),
        Some(Signer::Bad)
    );

    // Imported by the block's creator as its log's leading run, the history verifies here.
    assert_eq!(
        elsewhere
            .verify_imported(new, "test.app", &history.entries[0])
            .await
            .unwrap(),
        Some(Signer::Verified)
    );
    assert_eq!(
        elsewhere
            .verify_imported(new, "test.app", &history.entries[3])
            .await
            .unwrap(),
        None,
        "the block's own entry is no history"
    );
    // Replayed into the log after an entry of the block's own, an exported entry is no one's.
    let key = elsewhere.sealing_key(new).await.unwrap();
    elsewhere.push_imported(new, key, &saved.log[0]).unwrap();
    elsewhere.drain(|_| false).await.unwrap();
    let after = elsewhere.read_log(new, 4, 10).await.unwrap();
    assert!(after.entries[0].imported.is_some());
    assert_eq!(
        elsewhere
            .verify_imported(new, "test.app", &after.entries[0])
            .await
            .unwrap(),
        Some(Signer::Bad),
        "history may only lead the log"
    );
    harness.stop().await;
}
