use super::*;

#[test]
fn downstream_effect_recovers_after_memo_panics() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let value = create_signal(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let memo = create_memo(move || {
            let value = value.get();
            assert_ne!(value, 1);
            value * 2
        });
        let seen = seen.clone();
        create_effect(move || seen.borrow_mut().push(memo.get()));
    });
    value.set(1);
    assert!(catch_unwind(AssertUnwindSafe(flush)).is_err());
    value.set(2);
    flush();
    assert_eq!(*seen.borrow(), [0, 4]);
}
