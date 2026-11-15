use super::*;

use be_block_client::{Sealed, Write};
use be_block_commit::SealedThrough;
use be_block_protocol::JournalBase;
use be_block_session::{OpId, Submission};
use be_block_store::Hash;

fn tag() -> Hash {
    Hash::from_bytes([1; 32])
}

/// Numbers `count` operations on this device alone, as a replica offline does.
async fn numbered(peer: &Peer<MemoryStore>, block: Uuid, count: u64) {
    for counter in 1..=count {
        let submission = Submission {
            id: OpId { origin: 9, counter },
            base: 0,
            barrier: false,
            payload: vec![1],
        };
        peer.submit(block, tag(), submission, false, Vec::new())
            .await
            .expect("the entry is numbered");
    }
}

/// Seals new content on `parent`, holding the order through `through`.
async fn seal(
    peer: &Peer<MemoryStore>,
    block: Uuid,
    parent: Option<CommitId>,
    text: &str,
    through: u64,
) -> CommitId {
    let mut content = base();
    content.set(0, text);
    let write = Write::encoded(Sections::CONTENT_TYPE, content.parts(), parent)
        .sealed_through(Some(SealedThrough {
            sequence: through,
            offline: true,
            restarts: 0,
        }));
    match peer.seal(block, write).await.expect("the seal is kept") {
        Sealed::Sealed(commit) => commit,
        Sealed::Unchanged(_) => panic!("the content changed"),
    }
}

/// A device that stopped after its seal was kept and before it trimmed its journal opens on the
/// commit the seal was made on, with the seal as the head it takes once it replayed that far,
/// and only when its journal reaches the seal's last entry.
#[test]
fn a_journal_is_opened_on_the_commit_a_seal_was_made_on_only_when_it_reaches_the_seal() {
    with_test_types(|peer, _, _| async move {
        let block = block(&peer).await;
        let parent = peer.local_head(block);
        let base = JournalBase {
            commit: parent,
            sequence: 0,
            state: Vec::new(),
        };
        peer.rebase_journal(block, 0, tag(), base.clone(), false, false)
            .await
            .expect("the journal starts on the head");
        numbered(&peer, block, 4).await;

        let sealed = seal(&peer, block, parent, "sealed", 3).await;
        let (journal, kept, load_at, sealed_head) =
            crate::replicas::journal_for(&peer, block, None, false, Some(sealed));
        assert_eq!(load_at, parent, "it loads on the commit the seal was made on");
        assert_eq!(
            sealed_head.map(|head| (head.commit, head.through)),
            Some((sealed, 3))
        );
        assert!(kept, "the journal holds what only this device numbered");
        assert_eq!(journal.map(|journal| journal.entries.len()), Some(4));

        // A seal whose last entry the journal never reached.
        let short = seal(&peer, block, parent, "short", 5).await;
        let (journal, _, load_at, sealed_head) =
            crate::replicas::journal_for(&peer, block, None, false, Some(short));
        assert_eq!(
            (journal.is_some(), load_at, sealed_head),
            (false, Some(short), None),
            "a journal short of it is passed over"
        );

        // A journal with no entries through the seal holds nothing of it.
        let past = JournalBase { sequence: 4, ..base };
        peer.rebase_journal(block, 0, tag(), past, false, false)
            .await
            .expect("the journal starts past what it numbered");
        let empty = seal(&peer, block, parent, "empty", 5).await;
        let (_, _, load_at, sealed_head) =
            crate::replicas::journal_for(&peer, block, None, false, Some(empty));
        assert_eq!((load_at, sealed_head), (Some(empty), None), "the base alone");
    });
}
