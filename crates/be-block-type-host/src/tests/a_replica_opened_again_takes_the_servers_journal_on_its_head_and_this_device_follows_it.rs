use super::*;

use crate::replicas::{Reopening, reopening_journal};
use be_block_protocol::{Journal, JournalBase, JournalEntry};
use be_block_store::Hash;

fn tag() -> Hash {
    Hash::from_bytes([1; 32])
}

fn entries(generation: u64, sequences: &[u64], payload: u8) -> Vec<JournalEntry> {
    sequences
        .iter()
        .map(|sequence| JournalEntry {
            generation,
            sequence: *sequence,
            payload: vec![payload],
        })
        .collect()
}

fn held(journal: &Journal) -> Vec<(u64, u64, Vec<u8>)> {
    journal
        .entries
        .iter()
        .map(|entry| (entry.generation, entry.sequence, entry.payload.clone()))
        .collect()
}

/// A replica opened again on joining takes the server's journal on its head, and this device's
/// journal becomes it, so what the replica journals next follows the session's numbering. When
/// this device's held entries the server's lacks (6 to 8, journaled offline beside another
/// owner's 6 and 7, or entries on another commit), the replica is told those edits are lost to
/// it. With no server journal on the head, or one on another commit, the replica takes this
/// device's whole journal.
#[test]
fn a_replica_opened_again_takes_the_servers_journal_on_its_head_and_this_device_follows_it() {
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
        let journal = |generation, entries: Vec<JournalEntry>| {
            let (peer, base) = (&peer, base.clone());
            async move {
                peer.rebase_journal(block, generation, tag(), base, entries, false, false)
                    .await
                    .expect("the journal is kept");
            }
        };
        // The server's journal, sealed with the block's key as the server keeps it.
        journal(1, entries(1, &[6, 7], 0xB)).await;
        let server = (
            peer.sealed_journal(block).expect("sealed"),
            peer.local_journal(block).unwrap().expect("opened"),
        );
        let kept = || held(&peer.local_journal(block).unwrap().expect("kept"));

        // This device holds what the server's holds, and no more.
        journal(1, entries(1, &[6], 0xB)).await;
        let (taken, from, lost) = reopening_journal(&peer, block, Some(server.clone()), head);
        assert_eq!(from, Some(Reopening::Server));
        assert!(!lost);
        assert_eq!(held(&taken.expect("on the head")), held(&server.1));
        peer.append(
            block,
            1,
            tag(),
            entries(1, &[8], 0xC),
            false,
            false,
            Vec::new(),
        )
        .await
        .expect("the next edit is journaled");
        assert_eq!(
            kept(),
            vec![(1, 6, vec![0xB]), (1, 7, vec![0xB]), (1, 8, vec![0xC])],
            "this device's journal follows the server's numbering"
        );

        // This device journaled 6 to 8 of its own beside the server's.
        let own = entries(2, &[6, 7, 8], 0xA);
        journal(2, own.clone()).await;
        let (taken, from, _) = reopening_journal(&peer, block, None, head);
        assert_eq!(from, Some(Reopening::Device));
        assert_eq!(
            held(&taken.expect("on the head")),
            vec![(2, 6, vec![0xA]), (2, 7, vec![0xA]), (2, 8, vec![0xA])],
            "with no server journal on the head, this device's whole journal"
        );
        let (taken, from, lost) = reopening_journal(&peer, block, Some(server.clone()), head);
        assert_eq!(from, Some(Reopening::Server));
        assert_eq!(
            held(&taken.expect("on the head")),
            held(&server.1),
            "the server's journal, not this device's longer one"
        );
        assert!(
            lost,
            "the replica is told the edits only this device held are lost to it"
        );
        peer.append(
            block,
            1,
            tag(),
            entries(1, &[8], 0xD),
            false,
            false,
            Vec::new(),
        )
        .await
        .expect("the next edit is journaled");
        assert_eq!(
            kept(),
            vec![(1, 6, vec![0xB]), (1, 7, vec![0xB]), (1, 8, vec![0xD])],
            "this device's journal is the server's, and holds the next edit"
        );

        // This device's journal on another commit, holding entries the server's lacks.
        let elsewhere = JournalBase {
            commit: None,
            sequence: 2,
            state: Vec::new(),
        };
        peer.rebase_journal(
            block,
            3,
            tag(),
            elsewhere,
            entries(3, &[3], 0xE),
            false,
            false,
        )
        .await
        .expect("the journal is kept");
        let (taken, from, lost) = reopening_journal(&peer, block, Some(server.clone()), head);
        assert_eq!(from, Some(Reopening::Server));
        assert_eq!(held(&taken.expect("on the head")), held(&server.1));
        assert!(lost, "entries on another commit are reported lost too");
        assert_eq!(
            kept(),
            held(&server.1),
            "this device's journal is the server's"
        );

        let mut other = server.clone();
        other.0.base.commit = None;
        other.1.base.commit = None;
        let (taken, from, _) = reopening_journal(&peer, block, Some(other), head);
        assert_eq!(
            from,
            Some(Reopening::Device),
            "a server journal on another commit"
        );
        assert_eq!(held(&taken.expect("on the head")), held(&server.1));
    });
}
