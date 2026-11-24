use super::*;

/// A reader outside the session replays a journal on the head (`replay`): a restart in it,
/// whose floor is its own number so its base is always below it, is not skipped as one the
/// order passed over. Replay cannot reload its commit, so it fails, and the reader reads the
/// head alone instead of edits made on the restart applied to what came before it.
#[test]
fn replay_outside_a_session_refuses_a_journal_holding_a_restart() {
    let restart = crate::wire::Submitted {
        through: 0,
        entry: crate::wire::Entry::Reload {
            commit: CommitId::from_hash(Hash::of(b"the seal")),
            head: true,
            replace: false,
            restart: true,
            state: be_block_session::Payload::Inline(Vec::new()),
        },
    };
    let journal = be_sdk_type_api::Journal {
        base: be_sdk_type_api::JournalBase {
            commit: None,
            sequence: 0,
            state: Vec::new(),
        },
        entries: vec![be_sdk_type_api::JournalEntry {
            sequence: 1,
            origin: 1,
            counter: 1,
            base: 0,
            floor: 1,
            received: None,
            payload: restart.encode(),
        }],
    };
    assert!(crate::replica::replay::<ProseContent>(None, &journal).is_err());
}
