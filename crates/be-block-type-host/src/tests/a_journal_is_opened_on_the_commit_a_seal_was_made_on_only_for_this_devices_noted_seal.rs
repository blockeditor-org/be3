use super::*;

use be_block_client::{Sealed, Write};
use be_block_commit::SealedThrough;
use be_block_protocol::{JournalBase, JournalEntry};
use be_block_store::Hash;

const GENERATION: u64 = 7;

fn tag() -> Hash {
    Hash::from_bytes([1; 32])
}

/// Journals `sequences` in `generation` on this device alone, as an owner offline does.
async fn journaled(peer: &Peer<MemoryStore>, block: Uuid, generation: u64, sequences: &[u64]) {
    let entries = sequences
        .iter()
        .map(|sequence| JournalEntry {
            generation,
            sequence: *sequence,
            payload: vec![1],
        })
        .collect();
    peer.append(block, generation, tag(), entries, false, false, Vec::new())
        .await
        .expect("the entries are journaled");
}

/// Seals new content on `parent`, holding the session through `through`.
async fn seal(
    peer: &Peer<MemoryStore>,
    block: Uuid,
    parent: Option<CommitId>,
    text: &str,
    through: u64,
) -> CommitId {
    let mut content = base();
    content.set(0, text);
    let write = Write::encoded(Sections::CONTENT_TYPE, content.parts(), parent).sealed_through(
        Some(SealedThrough {
            generation: GENERATION,
            sequence: through,
        }),
    );
    match peer.seal(block, write).await.expect("the seal is kept") {
        Sealed::Sealed(commit) => commit,
        Sealed::Unchanged(_) => panic!("the content changed"),
    }
}

/// A device that stopped after the server took its seal and before it heard so opens on the
/// commit the seal was made on, and only then: the seal must be the one its journal noted, and
/// its journal must hold the seal's last entry in the seal's generation.
#[test]
fn a_journal_is_opened_on_the_commit_a_seal_was_made_on_only_for_this_devices_noted_seal() {
    with_test_types(|peer, _, _| async move {
        let block = block(&peer).await;
        let parent = peer.local_head(block);
        let base = JournalBase {
            commit: parent,
            sequence: 0,
            state: Vec::new(),
        };
        peer.rebase_journal(block, GENERATION, tag(), base.clone(), Vec::new(), false, false)
            .await
            .expect("the journal starts on the head");
        journaled(&peer, block, GENERATION, &[1, 2, 3, 4]).await;

        let sealed = seal(&peer, block, parent, "sealed", 3).await;
        let (journal, kept, load_at, sealed_head) =
            crate::replicas::journal_for(&peer, block, None, Some(sealed));
        assert_eq!(load_at, parent, "it loads on the commit the seal was made on");
        assert_eq!(
            sealed_head.map(|head| (head.commit, head.through)),
            Some((sealed, 3))
        );
        assert!(kept, "the journal holds an entry past the seal");
        assert_eq!(journal.map(|journal| journal.entries.len()), Some(4));

        // A seal this device did not note, though it holds the same place in a session.
        let other = seal(&peer, block, parent, "elsewhere", 3).await;
        peer.rebase_journal(block, GENERATION, tag(), base.clone(), Vec::new(), false, false)
            .await
            .expect("the journal starts over");
        journaled(&peer, block, GENERATION, &[1, 2, 3]).await;
        let (_, _, load_at, sealed_head) =
            crate::replicas::journal_for(&peer, block, None, Some(other));
        assert_eq!((load_at, sealed_head), (Some(other), None), "a seal not noted here");

        // A noted seal whose sequence the journal never reached, in a reused generation.
        let reused = seal(&peer, block, parent, "reused", 5).await;
        let (_, _, load_at, sealed_head) =
            crate::replicas::journal_for(&peer, block, None, Some(reused));
        assert_eq!((load_at, sealed_head), (Some(reused), None), "a journal short of it");

        // A journal with no entries through the seal holds nothing of it.
        peer.rebase_journal(block, GENERATION, tag(), base, Vec::new(), false, false)
            .await
            .expect("the journal starts over");
        let empty = seal(&peer, block, parent, "empty", 0).await;
        let (_, _, load_at, sealed_head) =
            crate::replicas::journal_for(&peer, block, None, Some(empty));
        assert_eq!((load_at, sealed_head), (Some(empty), None), "the base alone");
    });
}
