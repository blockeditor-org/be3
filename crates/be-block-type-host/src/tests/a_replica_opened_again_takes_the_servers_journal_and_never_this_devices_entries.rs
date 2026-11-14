use super::*;

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

/// This device journaled 6 to 8 in a session it made up offline while another device, owning
/// the session, journaled its own 6 and 7 on the server, both on the same head. Opened again on
/// rejoining, the replica takes the server's journal, however many entries this device's
/// holds: its own numbering would diverge from the session's. With no server journal on the
/// head it takes this device's base alone (the session's state, which numbers an editor's next
/// edit): what this device sequenced since is sealed before it rejoins, or given again from what
/// its editors sent, and taken from the journal too it would be applied twice.
#[test]
fn a_replica_opened_again_takes_the_servers_journal_and_never_this_devices_entries() {
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
        let server = Journal {
            tag: tag(),
            base: base.clone(),
            entries: entries(1, &[6, 7], 0xB),
            lost: false,
        };

        let taken = crate::replicas::reopening_journal(&peer, block, Some(server), head)
            .expect("a journal on the head");
        let taken: Vec<(u64, u64, Vec<u8>)> = taken
            .entries
            .into_iter()
            .map(|entry| (entry.generation, entry.sequence, entry.payload))
            .collect();
        assert_eq!(
            taken,
            vec![(1, 6, vec![0xB]), (1, 7, vec![0xB])],
            "the server's journal, not this device's longer one"
        );

        let own = crate::replicas::reopening_journal(&peer, block, None, head)
            .expect("this device's journal is on the head");
        assert_eq!(own.base.sequence, 5);
        assert_eq!(own.base.state, b"positions");
        assert!(own.entries.is_empty(), "none of this device's entries");

        let elsewhere = Journal {
            tag: tag(),
            base: JournalBase {
                commit: None,
                ..base
            },
            entries: entries(1, &[6], 0xB),
            lost: false,
        };
        let taken = crate::replicas::reopening_journal(&peer, block, Some(elsewhere), head)
            .expect("this device's journal is on the head");
        assert!(
            taken.entries.is_empty(),
            "a server journal on another commit is not taken"
        );
    });
}
