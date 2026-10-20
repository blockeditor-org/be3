use super::*;

#[test]
fn untracked_reads_do_not_subscribe() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let trigger = create_signal(0);
    let incidental = create_signal(1);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || {
            trigger.get();
            seen.borrow_mut()
                .push(incidental.get_untracked() + incidental.get_untracked());
        });
    });
    incidental.set(2);
    trigger.set(1);
    assert_eq!(*seen.borrow(), [2, 4]);
}
