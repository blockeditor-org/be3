use super::*;

#[test]
fn a_commit_chain_walks_to_its_root() {
    let store = store();
    let root = commit(&store, "one\n", 1_000, None);
    let middle = commit(&store, "one\ntwo\n", 2_000, Some(root));
    let head = commit(&store, "one\ntwo\nthree\n", 3_000, Some(middle));

    assert_eq!(
        store.first_parent_chain(head).unwrap(),
        vec![head, middle, root]
    );
    assert_eq!(store.read(head).unwrap(), b"one\ntwo\nthree\n");
    assert_eq!(store.read_range(head, 4, 4).unwrap(), b"two\n");
    assert!(store.is_ancestor(root, head).unwrap());
    assert!(!store.is_ancestor(head, root).unwrap());
    assert_eq!(store.get(head).unwrap().time, 3_000);
    assert_eq!(store.get(head).unwrap().kind, CommitKind::Autosave);
}
