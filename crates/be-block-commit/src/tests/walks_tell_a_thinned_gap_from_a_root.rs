use super::*;

use be_block_store::ObjectStore;

/// Retention thins a block's early history (one commit a minute in its first hour), so a walk
/// can meet a commit the store lacks: past it lies more history, not a root.
#[test]
fn walks_tell_a_thinned_gap_from_a_root() {
    let store = store();
    let root = commit(&store, "a\n", 1_000, None);
    let shared = commit(&store, "a\nb\n", 2_000, Some(root));
    let thinned = commit(&store, "a\nb\nc\n", 3_000, Some(shared));
    let theirs = commit(&store, "a\nb\nc\ntheirs\n", 4_000, Some(thinned));
    let ours = commit(&store, "a\nb\nours\n", 3_100, Some(shared));
    store.vault().store().remove(thinned.hash()).unwrap();

    assert_eq!(
        store.is_ancestor(shared, theirs).unwrap(),
        Descent::Unreached
    );
    assert_eq!(store.is_ancestor(ours, theirs).unwrap(), Descent::Unreached);
    assert_eq!(store.is_ancestor(root, ours).unwrap(), Descent::Yes);
    assert_eq!(
        store.common_ancestor(ours, theirs).unwrap(),
        MergeBase::Unreached
    );
    assert_eq!(
        store.common_ancestor(theirs, ours).unwrap(),
        MergeBase::Unreached
    );

    // A whole history is still a root's.
    let unrelated = commit(&store, "elsewhere\n", 5_000, None);
    assert_eq!(store.is_ancestor(unrelated, ours).unwrap(), Descent::No);
    assert_eq!(
        store.common_ancestor(ours, unrelated).unwrap(),
        MergeBase::Unrelated
    );
}
