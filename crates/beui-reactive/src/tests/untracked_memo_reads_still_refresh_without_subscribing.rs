use super::*;

#[test]
fn untracked_memo_reads_still_refresh_without_subscribing() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let value = create_signal(1);
    let trigger = create_signal(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let memo = create_memo(move || value.get() * 2);
        let seen = seen.clone();
        create_effect(move || {
            trigger.get();
            seen.borrow_mut().push(memo.get_untracked());
        });
    });
    value.set(2);
    flush();
    trigger.set(1);
    flush();
    assert_eq!(*seen.borrow(), [2, 4]);
}
