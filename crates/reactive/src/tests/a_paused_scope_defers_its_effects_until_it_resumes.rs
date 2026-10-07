use super::*;

#[test]
fn a_paused_scope_defers_its_effects_until_it_resumes() {
    let (count, set_count) = create_signal(0);
    let computed = Rc::new(Cell::new(0));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let scope = Scope::detached().pausable();
    scope.run(clone!(computed seen -> move || {
        let doubled = create_memo(move || {
            computed.set(computed.get() + 1);
            count.get() * 2
        });
        create_effect(move || seen.borrow_mut().push(doubled.get()));
    }));
    assert_eq!(*seen.borrow(), vec![0]);

    scope.pause();
    set_count.set(1);
    set_count.set(2);
    set_count.set(3);
    assert_eq!(*seen.borrow(), vec![0], "a paused scope's effects wait");
    assert_eq!(
        computed.get(),
        1,
        "nothing pulls a memo only a waiting effect reads"
    );

    scope.resume();
    assert_eq!(
        *seen.borrow(),
        vec![0, 6],
        "resuming runs a waiting effect once, with the latest value"
    );
    assert_eq!(computed.get(), 2);

    set_count.set(4);
    assert_eq!(*seen.borrow(), vec![0, 6, 8]);
}
