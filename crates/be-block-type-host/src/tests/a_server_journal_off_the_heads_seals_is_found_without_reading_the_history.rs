use super::*;

use be_block_client::{Sealed, Write};
use be_block_commit::SealedThrough;
use be_block_protocol::{Journal, JournalBase, JournalEntry};
use be_block_store::Hash;

fn tag() -> Hash {
    Hash::from_bytes([1; 32])
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
    let write = Write::encoded(Sections::CONTENT_TYPE, content.parts(), parent).sealed_through(
        Some(SealedThrough {
            sequence: through,
            offline: false,
            restarts: 0,
        }),
    );
    match peer.seal(block, write).await.expect("the seal is kept") {
        Sealed::Sealed(commit) => commit,
        Sealed::Unchanged(_) => panic!("the content changed"),
    }
}

/// The server's journal on `base` at `sequence`, holding the entries after it through `through`.
fn journal(base: Option<CommitId>, sequence: u64, through: u64) -> Journal {
    Journal {
        tag: tag(),
        base: JournalBase {
            commit: base,
            sequence,
            state: Vec::new(),
        },
        entries: (sequence + 1..=through)
            .map(|sequence| JournalEntry {
                sequence,
                origin: 9,
                counter: sequence,
                base: 0,
                floor: 0,
                received: Some(100),
                payload: vec![1],
            })
            .collect(),
        lost: false,
    }
}

/// Opens at `head` on `server`, answering what it loads on and its sealed head, and how many
/// commits it read as seals to find them.
fn opened(
    peer: &Peer<MemoryStore>,
    block: Uuid,
    server: Journal,
    head: CommitId,
) -> (Option<CommitId>, Option<(CommitId, u64)>, u64) {
    crate::replicas::SEALS_READ.with(|read| read.set(0));
    let (_, _, load_at, sealed_head) =
        crate::replicas::journal_for(peer, block, Some(server), true, Some(head));
    let read = crate::replicas::SEALS_READ.with(std::cell::Cell::get);
    (
        load_at,
        sealed_head.map(|sealed| (sealed.commit, sealed.through)),
        read,
    )
}

/// Opening on the server's journal looks down the head's seals for the one made on the
/// journal's base, but only as far as the base's number: a block's history is long, and a head
/// that lags behind the base, or a base off its seals, is answered from the head's own seal.
#[test]
fn a_server_journal_off_the_heads_seals_is_found_without_reading_the_history() {
    with_test_types(|peer, _, _| async move {
        let block = block(&peer).await;
        let root = peer.local_head(block);
        let mut chain = Vec::new();
        let mut parent = root;
        for sequence in 1..=40 {
            let commit = seal(&peer, block, parent, &format!("seal {sequence}"), sequence).await;
            chain.push(commit);
            parent = Some(commit);
        }
        let head = chain[39];
        let off = seal(&peer, block, root, "elsewhere", 37).await;

        assert_eq!(
            opened(&peer, block, journal(Some(off), 37, 39), head),
            (Some(off), None, 4),
            "a base off the head's seals: read down to the base's number"
        );
        assert_eq!(
            opened(&peer, block, journal(Some(off), 37, 39), chain[9]),
            (Some(off), None, 1),
            "a head behind the base: only the head is read"
        );
        assert_eq!(
            opened(&peer, block, journal(Some(chain[19]), 20, 22), head),
            (Some(chain[19]), Some((chain[20], 21)), 20),
            "the seal made on the base, found below the head"
        );
    });
}
