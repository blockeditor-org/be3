use super::*;

use be_block_protocol::{Journal, JournalBase};
use be_block_session::{OpId, Submission};
use be_block_store::Hash;

fn tag() -> Hash {
    Hash::from_bytes([1; 32])
}

fn held(journal: &Journal) -> Vec<(u64, Option<i64>, Vec<u8>)> {
    journal
        .entries
        .iter()
        .map(|entry| (entry.sequence, entry.received, entry.payload.clone()))
        .collect()
}

/// Starts this device's journal on `base` and numbers one entry per payload in it, offline.
async fn numbered(peer: &Peer<MemoryStore>, block: Uuid, base: &JournalBase, payloads: &[u8]) {
    peer.rebase_journal(block, 0, tag(), base.clone(), false, false)
        .await
        .expect("the journal starts");
    for (counter, payload) in payloads.iter().enumerate() {
        let submission = Submission {
            id: OpId {
                origin: 9,
                counter: counter as u64 + 1,
            },
            base: base.sequence,
            barrier: false,
            payload: vec![*payload],
        };
        peer.submit(block, tag(), submission, false, Vec::new())
            .await
            .expect("the entry is numbered");
    }
}

/// A replica opened again takes the server's journal, not this device's, and this device keeps
/// the server's from then on, so a later open offline replays what the server ordered. A server
/// journal on a commit other than the head is opened on its own base, where the order goes on.
#[test]
fn a_replica_opened_again_takes_the_servers_journal_and_this_device_keeps_it() {
    with_test_types(|peer, _, _| async move {
        let block = block(&peer).await;
        let head = match peer.save(block, &base(), None).await {
            Ok(be_block_client::Saved::Published(head)) => Some(head),
            saved => panic!("the block publishes: {saved:?}"),
        };
        let base = JournalBase {
            commit: head,
            sequence: 5,
            state: b"positions".to_vec(),
        };
        // The server's journal, sealed with the block's key as the server keeps it, each entry
        // stamped with when the server took it.
        numbered(&peer, block, &base, &[0xB, 0xB]).await;
        let mut sealed = peer.sealed_journal(block).expect("sealed");
        for entry in &mut sealed.entries {
            entry.received = Some(100);
        }
        let server = peer.open_journal(sealed.clone()).expect("opened");

        // This device numbered three of its own offline.
        numbered(&peer, block, &base, &[0xA, 0xA, 0xA]).await;
        let (journal, kept, load_at, _) =
            crate::replicas::journal_for(&peer, block, None, false, head);
        assert!(
            kept,
            "offline, this device's journal holds what the server lacks"
        );
        assert_eq!(load_at, head);
        assert_eq!(journal.map(|journal| journal.entries.len()), Some(3));

        let (journal, kept, load_at, _) =
            crate::replicas::journal_for(&peer, block, Some(server.clone()), true, head);
        assert_eq!(load_at, head);
        assert!(!kept, "the server holds what it hands over");
        assert_eq!(held(&journal.expect("the server's")), held(&server));

        peer.keep_journal(block, &sealed).expect("kept");
        let kept_here = peer.local_journal(block).unwrap().expect("kept");
        assert_eq!(
            held(&kept_here),
            held(&server),
            "this device keeps the server's journal"
        );
        let (journal, kept, _, _) = crate::replicas::journal_for(&peer, block, None, false, head);
        assert!(!kept);
        assert_eq!(held(&journal.expect("this device's")), held(&server));

        let mut other = server.clone();
        other.base.commit = None;
        let (journal, _, load_at, sealed_head) =
            crate::replicas::journal_for(&peer, block, Some(other), true, head);
        assert_eq!(
            (load_at, sealed_head),
            (None, None),
            "a server journal on another commit is opened on its base"
        );
        assert_eq!(held(&journal.expect("the server's")), held(&server));
    });
}
