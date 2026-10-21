use super::*;

#[test]
fn the_write_that_pauses_a_scope_still_reaches_its_effects() {
    let _root = RootScope::new();
    let open = create_signal(true);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let outer = Scope::new();
    outer.run({
        let seen = seen.clone();
        move || {
            let inner = Rc::new(Scope::detached().pausable());
            create_effect({
                let inner = inner.clone();
                move || match open.get() {
                    true => inner.resume(),
                    false => inner.pause(),
                }
            });
            inner.run(move || {
                create_effect(move || seen.borrow_mut().push(open.get()));
            });
            on_cleanup(move || drop(inner));
        }
    });
    assert_eq!(*seen.borrow(), vec![true]);

    open.set(false);
    flush();
    assert_eq!(
        *seen.borrow(),
        vec![true, false],
        "an effect already woken when its scope pauses runs, so it can close what it opened"
    );
}
