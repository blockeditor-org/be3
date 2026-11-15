use be_block_commit::CommitId;
use be_block_protocol::{JournalBase, JournalEntry};
use be_block_store::Hash;

use super::*;

/// A rebase drops the seal the journal noted, and the head it writes then has none: read back,
/// the journal still starts at its base and holds what came after it, so a device that stops
/// here replays those edits on that commit when it starts again.
#[test]
fn a_journal_rebased_with_no_seal_noted_reads_back_on_its_base() {
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
        generation: 1,
        sequence,
        payload: vec![u8::try_from(sequence).unwrap()],
    };
    let refs = database.scope(scope.clone()).unwrap();
    refs.append_journal(block, tag, 1, vec![entry(1), entry(2), entry(3)])
        .unwrap();
    refs.rebase_journal(
        block,
        tag,
        1,
        JournalBase {
            commit: Some(seal),
            sequence: 2,
            state: b"the state".to_vec(),
        },
        vec![entry(3)],
    )
    .unwrap();
    drop(refs);

    let reloaded = database.scope(scope).unwrap();
    let journal = reloaded.journal(block).expect("the journal is kept");
    assert_eq!(journal.tag, tag);
    assert_eq!(journal.base.commit, Some(seal));
    assert_eq!(journal.base.sequence, 2);
    assert_eq!(journal.base.state, b"the state");
    let entries: Vec<(u64, u64, Vec<u8>)> = journal
        .entries
        .into_iter()
        .map(|entry| (entry.generation, entry.sequence, entry.payload))
        .collect();
    assert_eq!(entries, vec![(1, 3, vec![3])]);
    assert_eq!(reloaded.sealing(block), None);
}
