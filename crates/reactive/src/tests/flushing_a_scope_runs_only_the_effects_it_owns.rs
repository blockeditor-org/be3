use super::*;

#[test]
fn flushing_a_scope_runs_only_the_effects_it_owns() {
    let _root = RootScope::new();
    let root = Scope::new();
    let value = create_signal(0);
    let (inside, outside) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let child = root.run(|| {
        let seen = Rc::clone(&outside);
        create_effect(move || seen.set(value.get()));
        let child = Scope::new();
        let seen = Rc::clone(&inside);
        child.run(|| {
            crate::child_scope(|| create_effect(move || seen.set(value.get())));
        });
        child
    });
    root.run(|| {
        value.set(1);
        child.flush_owned();
        assert_eq!(inside.get(), 1, "an effect under the scope runs");
        assert_eq!(
            outside.get(),
            0,
            "an effect outside the scope waits for the entry to end"
        );
    });
    assert_eq!(outside.get(), 1);
}
