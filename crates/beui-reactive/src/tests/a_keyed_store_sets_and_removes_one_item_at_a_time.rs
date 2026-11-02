use super::*;

#[test]
fn a_keyed_store_sets_and_removes_one_item_at_a_time() {
    let store = OwnedKeyedStore::<u32, String>::new();
    store.set(1, "one".to_owned());
    store.set(2, "two".to_owned());
    let one = store.get(&1);
    assert_eq!(
        store.keys().get_untracked(),
        [1, 2],
        "a new key joins the end"
    );

    store.set(1, "uno".to_owned());
    assert_eq!(
        store.keys().get_untracked(),
        [1, 2],
        "setting a key that is there keeps its place"
    );
    assert_eq!(one.get_untracked(), "uno", "in the same signal");

    store.remove(&1);
    assert_eq!(store.keys().get_untracked(), [2]);
    assert!(!store.contains(&1), "a removed key frees its item");
}
