use super::*;

#[test]
fn a_keyed_store_view_is_freed_with_the_scope_that_made_it() {
    let store = OwnedKeyedStore::<u32, u32>::new();
    store.reconcile_owned([(1, 10), (2, 20)]);
    let before = crate::arena::live();
    let scope = Scope::new();
    let total = scope.run(|| store.sum_by(|value| *value));
    scope.run(|| store.sorted_by_key(|value| *value));
    assert_eq!(total.get_untracked(), 30);
    assert_eq!(store.views(), 2);
    scope.dispose();
    flush();
    assert_eq!(store.views(), 0, "the store forgets the views");
    assert_eq!(crate::arena::live(), before, "and their outputs are freed");
    store.set(3, 30);

    let scope = Scope::new();
    let outlived = OwnedKeyedStore::<u32, u32>::new();
    let largest = scope.run(|| outlived.max_by(|value| *value));
    outlived.set(1, 1);
    assert_eq!(largest.get_untracked(), Some(1));
    drop(outlived);
    scope.dispose();
    flush();
}
