use super::*;

#[test]
fn panic_restores_tracking_and_batching() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let value = create_signal(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || seen.borrow_mut().push(value.get()));
    });
    assert!(
        catch_unwind(AssertUnwindSafe(|| batch(|| {
            value.set(1);
            untrack(|| panic!("abort batch"));
        })))
        .is_err()
    );
    value.set(2);
    assert_eq!(*seen.borrow(), [0, 2]);
}
