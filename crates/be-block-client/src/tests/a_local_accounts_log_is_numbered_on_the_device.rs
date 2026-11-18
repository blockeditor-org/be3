use super::*;

/// A local account's signed log has no server: each entry is numbered and stamped on the device
/// as it is appended, its head kept by the tree's right edge, and the log reads back the same
/// when the device starts again.
#[tokio::test]
async fn a_local_accounts_log_is_numbered_on_the_device() {
    let (account, organization) = (Uuid::now_v7(), Uuid::now_v7());
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let local = || {
        Peer::local(
            PeerConfig::new("", identity("local@example.com"), Credentials::Header)
                .chunker(ChunkerConfig::SMALL)
                .refs(refs.clone()),
            account,
            organization,
            store.clone(),
        )
        .unwrap()
    };
    let peer = local();
    let notes = workspace(&peer, "Notes").await;
    let mut seqs = Vec::new();
    for text in ["one", "two", "three"] {
        match peer.append(notes, text.as_bytes()).await.unwrap() {
            Appended::Logged { seq, .. } => seqs.push(seq),
            kept => panic!("a local account kept an entry to send: {kept:?}"),
        }
    }
    assert_eq!(seqs, [0, 1, 2]);
    assert_eq!(peer.pending(), refs::PendingCounts::default());
    drop(peer);

    let peer = local();
    let read = peer.read_log(notes, 0, 10).await.unwrap();
    let mut tree = be_block_keys::merkle::Tree::new();
    for logged in &read.entries {
        tree.push(logged.entry.leaf());
    }
    assert_eq!(read.head.size, 3);
    assert_eq!(*read.head.root.as_bytes(), tree.root());
    assert_eq!(
        read.entries
            .iter()
            .map(|logged| logged.opened.clone().unwrap())
            .collect::<Vec<_>>(),
        [&b"one"[..], b"two", b"three"]
    );
    let four = peer.append(notes, b"four").await.unwrap();
    assert!(matches!(four, Appended::Logged { seq: 3, .. }), "{four:?}");
    tree.push(peer.read_log(notes, 3, 1).await.unwrap().entries[0].entry.leaf());
    assert_eq!(
        peer.log_head(notes).map(|head| *head.root.as_bytes()),
        Some(tree.root())
    );
}
