use be_block_commit::CommitId;
use be_block_protocol::HistoryEntry;

use crate::revisions::listed;

fn commit(n: u8) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::from_bytes([n; 32]))
}

fn entry(n: u8, time: i64, head: bool) -> HistoryEntry {
    HistoryEntry {
        commit: commit(n),
        parents: Vec::new(),
        time,
        pinned: false,
        head,
    }
}

/// A commit kept on this device and not sent yet reaches the server later, with the head here
/// where it is: the history lists it, current, rather than the server's head before it.
#[test]
fn version_history_lists_the_commits_kept_here_with_the_head_here_current() {
    let server = [entry(1, 1_000, false), entry(2, 2_000, true)];
    let current = |listed: &[be_sdk_api::Revision]| -> Vec<(u8, bool)> {
        listed
            .iter()
            .map(|revision| (revision.commit[0], revision.current))
            .collect()
    };

    // Kept here, in the same millisecond as the server's head, as seals of one stretch of
    // typing are with the clock stopped.
    let shown = listed(&server, &[(commit(3), 2_000)], Some(commit(3)));
    assert_eq!(current(&shown), [(3, true), (2, false), (1, false)]);

    // Sent meanwhile: the server lists it too, once.
    let server = [
        entry(1, 1_000, false),
        entry(2, 2_000, false),
        entry(3, 2_000, true),
    ];
    let shown = listed(&server, &[(commit(3), 2_000)], Some(commit(3)));
    assert_eq!(current(&shown), [(3, true), (2, false), (1, false)]);

    // A head here that lags the server's, with nothing kept: another device published since,
    // and the server's head is current.
    let shown = listed(&server, &[], Some(commit(2)));
    assert_eq!(current(&shown), [(3, true), (2, false), (1, false)]);

    // A block this device does not hold: the server's head is current.
    let shown = listed(&server, &[], None);
    assert_eq!(current(&shown), [(3, true), (2, false), (1, false)]);
}
