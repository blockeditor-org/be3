use super::*;

#[test]
fn a_panicking_effect_can_run_again() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let value = create_signal(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || {
            let value = value.get();
            assert_ne!(value, 1);
            seen.borrow_mut().push(value);
        });
    });
    value.set(1);
    assert!(catch_unwind(AssertUnwindSafe(flush)).is_err());
    value.set(2);
    flush();
    assert_eq!(*seen.borrow(), [0, 2]);
}
