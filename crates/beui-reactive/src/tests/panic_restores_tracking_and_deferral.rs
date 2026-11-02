use super::*;

#[test]
fn panic_restores_tracking_and_deferral() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let value = create_signal(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || seen.borrow_mut().push(value.get()));
    });
    assert!(
        catch_unwind(AssertUnwindSafe(|| scope.run(|| {
            value.set(1);
            untrack(|| panic!("abort entry"));
        })))
        .is_err()
    );
    assert_eq!(*seen.borrow(), [0], "an entry that panics does not flush");
    value.set(2);
    flush();
    assert_eq!(*seen.borrow(), [0, 2]);
}
