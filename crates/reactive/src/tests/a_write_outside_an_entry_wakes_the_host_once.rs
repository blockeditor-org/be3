use super::*;

#[test]
fn a_write_outside_an_entry_wakes_the_host_once() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let value = create_signal(0);
    let seen = Rc::new(Cell::new(0));
    let wakes = Rc::new(Cell::new(0));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || seen.set(value.get()));
    });
    crate::wake_with({
        let wakes = wakes.clone();
        move || wakes.set(wakes.get() + 1)
    });
    scope.run(|| value.set(1));
    assert_eq!(wakes.get(), 0, "a write inside an entry needs no wake");
    assert_eq!(seen.get(), 1);
    value.set(2);
    value.set(3);
    assert_eq!(wakes.get(), 1, "a burst of writes wakes once");
    assert_eq!(seen.get(), 1);
    flush();
    assert_eq!(seen.get(), 3);
    value.set(4);
    assert_eq!(wakes.get(), 2, "the next write after a flush wakes again");
}
