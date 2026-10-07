use super::*;

#[test]
fn a_nested_pausable_scope_waits_while_any_scope_around_it_is_paused() {
    let (count, set_count) = create_signal(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let outer = Scope::detached().pausable();
    let inner = outer.run(clone!(seen -> move || {
        let inner = Scope::detached().pausable();
        inner.run(move || {
            create_effect(move || seen.borrow_mut().push(count.get()));
        });
        inner
    }));

    outer.pause();
    inner.pause();
    set_count.set(1);
    inner.resume();
    assert_eq!(*seen.borrow(), vec![0], "the outer scope is still paused");

    outer.resume();
    assert_eq!(*seen.borrow(), vec![0, 1]);
}
