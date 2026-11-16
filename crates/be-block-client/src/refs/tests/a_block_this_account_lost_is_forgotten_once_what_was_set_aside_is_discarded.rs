use be_block_commit::CommitId;
use be_block_store::Hash;
use uuid::Uuid;

use super::*;

/// A block this account lost access to, with edits kept beside its outbox (a replica's kept
/// edits and an entry this device numbered offline): they keep it from being forgotten while its
/// unsent edits are set aside, and once those are discarded (`Peer::discard_unsent`, which drops
/// the local edits of a block whose access was lost) it is forgotten, nothing left behind.
#[test]
fn a_block_this_account_lost_is_forgotten_once_what_was_set_aside_is_discarded() {
    let path = std::env::temp_dir().join(format!("be-client-refs-{}.sqlite3", Uuid::new_v4()));
    let scope = Scope {
        server: "server".to_owned(),
        account: Uuid::new_v4(),
        organization: Uuid::new_v4(),
    };
    let refs = RefsDb::open(&path).unwrap().scope(scope).unwrap();
    let block = Uuid::new_v4();
    refs.keep_open_edits(
        block,
        OpenEdits {
            origin: 1,
            edits: vec![vec![1, 2, 3]],
            ..OpenEdits::default()
        },
    )
    .unwrap();
    refs.number_offline(
        block,
        Hash::of(b"tag"),
        be_block_session::Submission {
            id: be_block_session::OpId {
                origin: 1,
                counter: 1,
            },
            base: 0,
            barrier: false,
            payload: vec![4],
        },
    )
    .unwrap();
    refs.push(Op::Publish(Pending {
        block,
        commit: CommitId::from_hash(Hash::of(b"commit")),
        expected: None,
        parents: Vec::new(),
        chunks: Vec::new(),
        time: 0,
        pinned: false,
        references: Vec::new(),
        metadata: Vec::new(),
    }))
    .unwrap();
    assert!(refs.move_to_unsent(block, 0, None).unwrap());
    let gone: HashSet<Uuid> = [block].into();
    assert_eq!(refs.forget(&gone).unwrap(), 0, "kept for what it holds");

    let unsent = refs.discard_unsent(block).unwrap().expect("set aside");
    assert_eq!(unsent.refused, None, "for lost access");
    assert!(refs.discarded(block), "its local edits went with them");
    assert!(
        !refs.keep_open_edits(block, OpenEdits::default()).unwrap(),
        "none are kept while it may not edit it"
    );
    assert_eq!(refs.forget(&gone).unwrap(), 1, "forgotten");
    assert!(!refs.discarded(block), "forgetting it clears the mark");
    assert!(refs.open_edits(block).is_none());
    assert!(refs.journal(block).is_none());
}
