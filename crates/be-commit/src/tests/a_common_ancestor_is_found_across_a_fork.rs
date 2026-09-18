use super::*;

#[test]
fn a_common_ancestor_is_found_across_a_fork() {
    let store = store();
    let root = commit(&store, "a\n", 1_000, None);
    let shared = commit(&store, "a\nb\n", 2_000, Some(root));
    let ours = commit(&store, "a\nb\nours\n", 3_000, Some(shared));
    let theirs = commit(&store, "a\nb\ntheirs\n", 3_100, Some(shared));

    assert_eq!(store.common_ancestor(ours, theirs).unwrap(), Some(shared));
    assert_eq!(store.common_ancestor(ours, shared).unwrap(), Some(shared));
    assert_eq!(store.common_ancestor(ours, ours).unwrap(), Some(ours));

    let merged = store
        .put(
            &Commit::new(store.get(ours).unwrap().manifest, AUTHOR, 4_000)
                .with_parents(vec![ours, theirs])
                .with_kind(CommitKind::Merge),
        )
        .unwrap();
    assert!(store.is_ancestor(theirs, merged).unwrap());
    assert!(store.is_ancestor(root, merged).unwrap());

    let unrelated = commit(&store, "elsewhere\n", 5_000, None);
    assert_eq!(store.common_ancestor(merged, unrelated).unwrap(), None);
}
