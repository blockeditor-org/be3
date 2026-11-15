use be_block_commit::CommitId;
use be_block_protocol::{JournalBase, JournalEntry};
use be_block_store::Hash;

use super::*;

/// A seal trims this device's journal as it trims the server's: read back, the journal starts
/// at the seal and holds what the order numbered after it, so a device that stops here replays
/// those edits on that commit when it starts again, and numbers on after them offline.
#[test]
fn a_rebased_journal_reads_back_on_its_base_with_what_came_after() {
    let database = refs::RefsDb::memory();
    let scope = refs::Scope {
        server: "server".to_owned(),
        account: Uuid::new_v4(),
        organization: Uuid::new_v4(),
    };
    let block = Uuid::new_v4();
    let tag = Hash::of(b"the counter's module");
    let seal = CommitId::from_hash(Hash::of(b"the seal"));
    let entry = |sequence| JournalEntry {
        sequence,
        origin: 1,
        counter: sequence,
        base: sequence - 1,
        floor: 0,
        received: Some(1_000),
        payload: vec![u8::try_from(sequence).unwrap()],
    };
    let refs = database.scope(scope.clone()).unwrap();
    refs.append_journal(block, tag, vec![entry(1), entry(2), entry(3)])
        .unwrap();
    refs.rebase_journal(
        block,
        tag,
        JournalBase {
            commit: Some(seal),
            sequence: 2,
            state: b"the state".to_vec(),
        },
    )
    .unwrap();
    drop(refs);

    let reloaded = database.scope(scope).unwrap();
    let journal = reloaded.journal(block).expect("the journal is kept");
    assert_eq!(journal.tag, tag);
    assert_eq!(journal.base.commit, Some(seal));
    assert_eq!(journal.base.sequence, 2);
    assert_eq!(journal.base.state, b"the state");
    let entries: Vec<(u64, Vec<u8>)> = journal
        .entries
        .into_iter()
        .map(|entry| (entry.sequence, entry.payload))
        .collect();
    assert_eq!(entries, vec![(3, vec![3])]);

    let offline = reloaded
        .number_offline(
            block,
            tag,
            be_block_session::Submission {
                id: be_block_session::OpId {
                    origin: 1,
                    counter: 4,
                },
                base: 3,
                barrier: false,
                payload: vec![4],
            },
        )
        .unwrap();
    assert_eq!(
        offline.sequence, 4,
        "offline, the order goes on from what it heard"
    );
    assert_eq!(offline.received, None);
}
