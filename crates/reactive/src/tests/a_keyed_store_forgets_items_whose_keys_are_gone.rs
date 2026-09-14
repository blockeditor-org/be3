use super::*;

#[test]
fn a_keyed_store_forgets_items_whose_keys_are_gone() {
    let store: KeyedStore<u32, u32> = KeyedStore::new();
    store.reconcile_owned((0..5).map(|index| (index, index * 10)));
    assert_eq!(store.tracked_keys(), 5);
    let kept = store.get(&3);
    store.reconcile_owned([(1u32, 10u32), (3, 30)]);
    assert_eq!(store.tracked_keys(), 2);
    assert!(!store.contains(&4));
    assert_eq!(kept.get_untracked(), 30);
    assert_eq!(store.keys().get_untracked(), [1, 3]);
}
