use super::*;

#[test]
fn a_stored_value_is_shared_by_its_copies_and_freed_with_its_scope() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let count = scope.run(|| store(Cell::new(0)));
    let bump = move || count.with(|count| count.set(count.get() + 1));
    bump();
    bump();
    assert_eq!(count.with(Cell::get), 2);
    let owned = OwnedStored::new(String::from("kept"));
    let handle = owned.handle();
    scope.dispose();
    assert!(!count.is_alive());
    assert_eq!(count.try_with(Cell::get), None);
    assert_eq!(*handle.get(), "kept");
    drop(owned);
    assert!(!handle.is_alive());
}
