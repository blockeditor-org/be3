use be_block_commit::CommitId;
use be_block_store::Hash;
use uuid::Uuid;

use super::*;

/// Discarding a block's unsent edits puts its local head back on the server's: kept for an op of
/// it that stays in the outbox (a definition), the block must not build on the discarded commit
/// once this account may edit it again.
#[test]
fn discarding_puts_the_head_back_on_the_servers() {
    let path = std::env::temp_dir().join(format!("be-client-refs-{}.sqlite3", Uuid::new_v4()));
    let scope = Scope {
        server: "server".to_owned(),
        account: Uuid::new_v4(),
        organization: Uuid::new_v4(),
    };
    let refs = RefsDb::open(&path).unwrap().scope(scope.clone()).unwrap();
    let block = Uuid::new_v4();
    let remote = CommitId::from_hash(Hash::of(b"remote"));
    let discarded = CommitId::from_hash(Hash::of(b"discarded"));
    refs.set_remote(block, remote, |_, _| false).unwrap();
    refs.push_publish(Pending {
        block,
        commit: discarded,
        expected: Some(remote),
        parents: vec![remote],
        chunks: Vec::new(),
        time: 0,
        pinned: false,
        references: Vec::new(),
        metadata: Vec::new(),
    })
    .unwrap();
    assert_eq!(refs.head(block), Some(discarded));
    assert!(refs.move_to_unsent(block, 0, None).unwrap());
    refs.push(Op::Define(Defining {
        block,
        metadata: vec![1],
        references: Vec::new(),
    }))
    .unwrap();

    refs.discard_unsent(block).unwrap().expect("set aside");
    assert_eq!(refs.head(block), Some(remote), "back on the server's head");
    assert!(
        refs.outbox()
            .iter()
            .all(|(_, op)| !matches!(op, Op::Publish(_))),
        "nothing sealed of it waits"
    );
    let reopened = RefsDb::open(&path).unwrap().scope(scope).unwrap();
    assert_eq!(reopened.head(block), Some(remote), "kept so");
}
