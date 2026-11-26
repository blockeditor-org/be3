use super::*;

/// The block file of a block saved with `log`.
fn file_of(id: Uuid, log: Vec<crate::ExportedEntry>) -> Vec<u8> {
    crate::transfer::encode(&[crate::transfer::SavedBlock {
        id,
        block_type: be_type_workspace::WorkspaceContent::CONTENT_TYPE,
        parent: None,
        metadata: be_block_content::BlockMetadata::named("Chat"),
        references: Vec::new(),
        content: None,
        log,
    }])
}

/// What a block's log reads back as: each entry's block it was written in (none for the
/// block's own) and what it says.
async fn read_back(peer: &Peer<MemoryStore>, block: Uuid) -> Vec<(Option<Uuid>, Vec<u8>)> {
    peer.read_log(block, 0, 10)
        .await
        .unwrap()
        .entries
        .iter()
        .map(|logged| {
            (
                logged.imported.as_ref().map(|original| original.block),
                logged.bytes().unwrap_or_default().to_vec(),
            )
        })
        .collect()
}

/// A chat imported with history and exported again carries that history in the file, each
/// entry naming the chat it was written in, ahead of what was written in the chat exported:
/// imported once more, the new chat shows both as history, and each verifies against the chat
/// it was written in. A file whose history follows the block's own entries is refused.
#[tokio::test]
async fn a_chats_imported_history_survives_a_second_export_and_import() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let first = workspace(&owner, "Chat").await;
    for said in ["one", "two"] {
        owner
            .append(
                first,
                "test.app",
                said.as_bytes(),
                be_block_content::Along::default(),
            )
            .await
            .unwrap();
    }

    // Exported and imported once: the history, then a message of the new chat's own.
    let file = file_of(first, owner.export_log(first).await.unwrap());
    let mut copy = crate::transfer::renumber(crate::transfer::decode(&file).unwrap());
    let second = copy[0].id;
    assert!(!import_one(&owner, copy.remove(0)).await.is_empty());
    owner.drain(|_| false).await.unwrap();
    owner
        .append(
            second,
            "test.app",
            b"three",
            be_block_content::Along::default(),
        )
        .await
        .unwrap();

    // Exported again: the history goes along, each entry as written in the first chat.
    let log = owner.export_log(second).await.unwrap();
    assert_eq!(
        log.iter().map(|entry| entry.block).collect::<Vec<_>>(),
        [first, first, second],
        "the file carries the history ahead of the chat's own"
    );
    let file = file_of(second, log.clone());
    let mut copy = crate::transfer::renumber(crate::transfer::decode(&file).unwrap());
    let third = copy[0].id;
    assert!(!import_one(&owner, copy.remove(0)).await.is_empty());
    owner.drain(|_| false).await.unwrap();

    assert_eq!(
        read_back(&owner, third).await,
        [
            (Some(first), b"one".to_vec()),
            (Some(first), b"two".to_vec()),
            (Some(second), b"three".to_vec()),
        ],
        "the second import shows both chats' messages as history"
    );
    let history = owner.read_log(third, 0, 10).await.unwrap();
    for logged in &history.entries {
        assert_eq!(
            owner
                .verify_imported(third, "test.app", logged)
                .await
                .unwrap(),
            Some(Signer::Verified),
            "each entry verifies against the chat it was written in"
        );
    }

    let mut out_of_order = log;
    out_of_order.rotate_left(2);
    assert!(
        crate::transfer::decode(&file_of(second, out_of_order)).is_err(),
        "history after the block's own entries is refused"
    );
    harness.stop().await;
}
