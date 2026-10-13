use super::*;

#[test]
fn a_keyed_store_frees_the_signals_of_keys_that_are_gone() {
    let before = crate::arena::live();
    let store = KeyedStore::<u32, u32>::new();
    store.reconcile_owned([(1, 10), (2, 20), (3, 30)]);
    assert_eq!(crate::arena::live(), before + 4);
    let gone = store.get(&2);
    store.reconcile_owned([(1, 10), (3, 30)]);
    assert_eq!(crate::arena::live(), before + 3);
    assert!(!gone.is_alive());
    drop(store);
    assert_eq!(
        crate::arena::live(),
        before,
        "dropping the store frees the rest"
    );
}
