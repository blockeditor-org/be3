use super::*;

#[test]
fn writes_defer_effects_until_a_flush_but_memo_reads_are_current() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let left = create_signal(1);
    let right = create_signal(2);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sum = scope.run(|| {
        let sum = create_memo(move || left.get() + right.get());
        let seen = seen.clone();
        create_effect(move || seen.borrow_mut().push(sum.get()));
        sum
    });
    left.set(10);
    assert_eq!(left.get_untracked(), 10, "a value updates at once");
    assert_eq!(sum.get(), 12);
    scope.run(|| {
        right.set(20);
        assert_eq!(*seen.borrow(), [3], "a nested entry does not flush");
    });
    assert_eq!(*seen.borrow(), [3, 30], "the entry flushes as it ends");
    right.set(5);
    assert_eq!(sum.get(), 15);
    assert_eq!(*seen.borrow(), [3, 30]);
    flush();
    assert_eq!(*seen.borrow(), [3, 30, 15]);
}
