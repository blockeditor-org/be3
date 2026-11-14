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

/// This device journaled 6 to 8 in a session it made up offline while another device, owning
/// the session, journaled its own 6 and 7 on the server, both on the same head. Opened again on
/// joining, the replica takes the server's journal, however many entries this device's holds,
/// and this device's journal becomes the server's: what it journals next follows the session's
/// numbering rather than being dropped beneath entries of its own. With no server journal on
/// the head it takes this device's whole journal; one on another commit is not taken.
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
        // The server's journal, sealed with the block's key as the server keeps it.
        peer.rebase_journal(
            block,
            1,
            tag(),
            base.clone(),
            entries(1, &[6, 7], 0xB),
            false,
            false,
        )
        .await
        .expect("the server's journal is sealed");
        let server = (
            peer.sealed_journal(block).expect("sealed"),
            peer.local_journal(block).unwrap().expect("opened"),
        );
        // This device's own, journaled offline beside it.
        peer.rebase_journal(block, 2, tag(), base.clone(), Vec::new(), false, false)
            .await
            .expect("the journal starts on the head");
        peer.append(
            block,
            2,
            tag(),
            entries(2, &[6, 7, 8], 0xA),
            false,
            false,
            Vec::new(),
        )
        .await
        .expect("this device journals 6 to 8");

        let (taken, from) = reopening_journal(&peer, block, Some(server.clone()), head);
        assert_eq!(from, Some(Reopening::Server));
        assert_eq!(
            held(&taken.expect("a journal on the head")),
            vec![(1, 6, vec![0xB]), (1, 7, vec![0xB])],
            "the server's journal, not this device's longer one"
        );
        // The replica journals its next edit after the server's entries.
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
        let kept = peer.local_journal(block).unwrap().expect("kept");
        assert_eq!(
            held(&kept),
            vec![(1, 6, vec![0xB]), (1, 7, vec![0xB]), (1, 8, vec![0xC])],
            "this device's journal follows the server's numbering"
        );

        let (own, from) = reopening_journal(&peer, block, None, head);
        assert_eq!(from, Some(Reopening::Device));
        assert_eq!(
            held(&own.expect("on the head")),
            held(&kept),
            "this device's whole journal"
        );

        let mut elsewhere = server.clone();
        elsewhere.0.base.commit = None;
        elsewhere.1.base.commit = None;
        let (taken, from) = reopening_journal(&peer, block, Some(elsewhere), head);
        assert_eq!(
            from,
            Some(Reopening::Device),
            "a server journal on another commit"
        );
        assert_eq!(held(&taken.expect("on the head")), held(&kept));
    });
}
