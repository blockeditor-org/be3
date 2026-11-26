use std::collections::HashMap;

use be_block_commit::CommitId;
use be_block_store::Hash;
use uuid::Uuid;

use super::*;

fn pending(block: Uuid, commit: CommitId, on: CommitId) -> Pending {
    Pending {
        block,
        commit,
        expected: Some(on),
        parents: vec![on],
        chunks: Vec::new(),
        time: 0,
        pinned: false,
        references: Vec::new(),
        metadata: Vec::new(),
        journal: None,
    }
}

fn id(name: &str) -> CommitId {
    CommitId::from_hash(Hash::of(name.as_bytes()))
}

/// The waiting commits of a block sealed again (`Peer::reseal`, `Peer::restamp`) take their
/// places only while the outbox holds what the rewrite read: a seal that landed meanwhile makes
/// it read again. Once they have, a seal on a commit they replaced is refused, to be sealed on
/// its replacement, and what replaced what is kept across a reopen.
#[test]
fn a_rewrite_of_waiting_commits_gives_way_to_a_seal_and_turns_later_seals_onto_it() {
    let path = std::env::temp_dir().join(format!("be-client-refs-{}.sqlite3", Uuid::new_v4()));
    let scope = Scope {
        server: "server".to_owned(),
        account: Uuid::new_v4(),
        organization: Uuid::new_v4(),
    };
    let refs = RefsDb::open(&path).unwrap().scope(scope.clone()).unwrap();
    let block = Uuid::new_v4();
    let (remote, first, again) = (id("remote"), id("first"), id("first again"));
    let Pushed::Queued(sequence) = refs.push_publish(pending(block, first, remote)).unwrap() else {
        panic!("the first seal is queued");
    };
    let moved = HashMap::from([(first, again)]);
    let rewritten = || vec![(sequence, first, pending(block, again, remote))];

    // A seal lands on the first commit after the rewrite read the outbox: the rewrite gives way.
    let second = id("second");
    let Pushed::Queued(_) = refs.push_publish(pending(block, second, first)).unwrap() else {
        panic!("the second seal is queued");
    };
    assert!(!refs.replace_publishes(block, rewritten(), &moved).unwrap());
    assert_eq!(refs.head(block), Some(second), "nothing was replaced");
    assert_eq!(refs.replaced(first), first);

    // Read again, with the second among them, the rewrite takes its place.
    let second_again = id("second again");
    let both = HashMap::from([(first, again), (second, second_again)]);
    let mut all = rewritten();
    let second_sequence = refs
        .pending_for(block)
        .into_iter()
        .find_map(|(sequence, op)| match op {
            Op::Publish(pending) if pending.commit == second => Some(sequence),
            _ => None,
        })
        .unwrap();
    all.push((second_sequence, second, pending(block, second_again, again)));
    assert!(refs.replace_publishes(block, all, &both).unwrap());
    assert_eq!(refs.head(block), Some(second_again));
    assert_eq!(refs.replaced(second), second_again);
    assert_eq!(refs.replaced_by(second_again), [second]);

    // A seal that read the old head before the rewrite and is queued after it is refused.
    assert_eq!(
        refs.push_publish(pending(block, id("late"), second)).unwrap(),
        Pushed::OnReplaced
    );
    assert_eq!(refs.head(block), Some(second_again));

    let reopened = RefsDb::open(&path).unwrap().scope(scope).unwrap();
    assert_eq!(reopened.replaced(first), again, "kept across a reopen");
}
