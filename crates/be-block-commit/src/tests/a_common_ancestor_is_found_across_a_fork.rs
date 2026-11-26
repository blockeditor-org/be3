use super::*;

#[test]
fn a_common_ancestor_is_found_across_a_fork() {
    let store = store();
    let root = commit(&store, "a\n", 1_000, None);
    let shared = commit(&store, "a\nb\n", 2_000, Some(root));
    let ours = commit(&store, "a\nb\nours\n", 3_000, Some(shared));
    let theirs = commit(&store, "a\nb\ntheirs\n", 3_100, Some(shared));

    assert_eq!(store.common_ancestor(ours, theirs).unwrap(),
        MergeBase::Base(shared));
    assert_eq!(store.common_ancestor(ours, shared).unwrap(),
        MergeBase::Behind);
    assert_eq!(store.common_ancestor(ours, ours).unwrap(), MergeBase::Behind);

    let merged = store
        .put(
            KEY,
            &Commit::new(store.get(ours).unwrap().manifest, AUTHOR, 4_000)
                .with_parents(vec![ours, theirs])
                .with_kind(CommitKind::Merge),
        )
        .unwrap();
    assert_eq!(store.is_ancestor(theirs, merged).unwrap(), Descent::Yes);
    assert_eq!(store.is_ancestor(root, merged).unwrap(), Descent::Yes);

    let unrelated = commit(&store, "elsewhere\n", 5_000, None);
    assert_eq!(store.common_ancestor(merged, unrelated).unwrap(),
        MergeBase::Unrelated);
}
