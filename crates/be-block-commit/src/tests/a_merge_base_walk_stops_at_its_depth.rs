use super::*;

use be_block_store::ObjectStore;

/// What the walk found, and how many commits it reached.
fn walk(
    store: &CommitStore<MemoryStore>,
    ours: CommitId,
    theirs: CommitId,
    depth: usize,
) -> (MergeBase, usize) {
    let mut walk = MergeBaseWalk::new(ours, theirs, depth);
    let mut steps = 0;
    loop {
        if let Some(found) = walk.step(store).unwrap() {
            return (found, walk.reached());
        }
        steps += 1;
        assert!(steps <= depth, "the walk reads at most {depth} levels");
    }
}

fn merge_base(
    store: &CommitStore<MemoryStore>,
    ours: CommitId,
    theirs: CommitId,
    depth: usize,
) -> MergeBase {
    walk(store, ours, theirs, depth).0
}

fn chain(store: &CommitStore<MemoryStore>, from: CommitId, length: usize, time: i64) -> CommitId {
    (0..length).fold(from, |parent, step| {
        commit(
            store,
            &format!("{time} {step}\n"),
            time + step as i64,
            Some(parent),
        )
    })
}

#[test]
fn a_merge_base_walk_stops_at_its_depth() {
    let store = store();
    let root = commit(&store, "a\n", 1_000, None);
    let shared = chain(&store, root, 3, 2_000);
    let ours = chain(&store, shared, 2, 3_000);
    let theirs = chain(&store, shared, 4, 4_000);

    assert_eq!(
        merge_base(&store, ours, theirs, 4),
        MergeBase::Base(shared),
        "a fork within the depth meets at its common ancestor"
    );
    assert_eq!(
        merge_base(&store, ours, theirs, 3),
        MergeBase::Unreached,
        "one side reaches the fork only past the depth"
    );
    assert_eq!(
        merge_base(&store, theirs, shared, 4),
        MergeBase::Behind,
        "an ancestor of ours is behind it"
    );
    assert_eq!(
        merge_base(&store, shared, theirs, 4),
        MergeBase::Base(shared),
        "a descendant of ours meets it at ours"
    );
    assert_eq!(merge_base(&store, ours, ours, 0), MergeBase::Behind);

    // A long history on both sides is walked only to the depth: each side reaches its head and
    // one commit a level.
    let long_ours = chain(&store, shared, 50, 6_000);
    let long_theirs = chain(&store, shared, 50, 7_000);
    assert_eq!(
        walk(&store, long_ours, long_theirs, 10),
        (MergeBase::Unreached, 2 * 11),
        "the walk reaches no commit past its depth"
    );
    assert_eq!(
        walk(&store, ours, theirs, 64),
        (MergeBase::Base(shared), 2 * 5),
        "a walk that meets stops at the level where it meets: both sides walk the four levels \
         back to the fork from theirs"
    );

    let unrelated = commit(&store, "elsewhere\n", 5_000, None);
    assert_eq!(
        merge_base(&store, ours, unrelated, 64),
        MergeBase::Unrelated,
        "histories that share nothing end before the depth"
    );

    // Retention thinned away a commit on their side: its chain ends there, though the fork
    // lies past it, so the walk cannot tell they are unrelated.
    let thinned = chain(&store, shared, 3, 8_000);
    let past_the_gap = chain(&store, thinned, 2, 9_000);
    store.vault().store().remove(thinned.hash()).unwrap();
    assert_eq!(
        merge_base(&store, long_ours, past_the_gap, 64),
        MergeBase::Unreached,
        "a chain cut short by thinning is not a root"
    );
}
