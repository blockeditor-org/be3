use super::*;

use be_store::ObjectStore;

#[test]
fn pruned_history_leaves_the_remaining_chain_walkable() {
    let store = store();
    let root = commit(&store, "one\n", 1_000, None);
    let middle = commit(&store, "one\ntwo\n", 2_000, Some(root));
    let head = commit(&store, "one\ntwo\nthree\n", 3_000, Some(middle));

    store.vault().store().remove(middle.hash()).unwrap();

    assert!(!store.has(middle).unwrap());
    assert_eq!(store.try_get(middle).unwrap(), None);
    assert_eq!(store.first_parent_chain(head).unwrap(), vec![head, middle]);
    assert_eq!(store.read(head).unwrap(), b"one\ntwo\nthree\n");
    assert!(!store.is_ancestor(root, head).unwrap());
}
