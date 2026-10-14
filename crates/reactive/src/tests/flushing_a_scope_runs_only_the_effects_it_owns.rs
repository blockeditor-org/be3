use super::*;

#[test]
fn flushing_a_scope_runs_only_the_effects_it_owns() {
    let _root = RootScope::new();
    let root = Scope::new();
    let (value, set_value) = create_signal(0);
    let (inside, outside) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let child = root.run(|| {
        let seen = Rc::clone(&outside);
        let watched = value;
        create_effect(move || seen.set(watched.get()));
        let child = Scope::new();
        let seen = Rc::clone(&inside);
        let watched = value;
        child.run(|| {
            crate::child_scope(|| create_effect(move || seen.set(watched.get())));
        });
        child
    });
    batch(|| {
        set_value.set(1);
        child.flush_owned();
        assert_eq!(inside.get(), 1, "an effect under the scope runs");
        assert_eq!(
            outside.get(),
            0,
            "an effect outside the scope waits for the batch"
        );
    });
    assert_eq!(outside.get(), 1);
}
