use super::*;

#[test]
fn a_child_scope_lives_as_long_as_the_scope_it_was_opened_in() {
    let _root = RootScope::new();
    let outer = Scope::new();
    let cleanups = Rc::new(Cell::new(0));
    let sink = cleanups.clone();
    let ((), child) =
        outer.run(|| crate::child_scope(move || on_cleanup(move || sink.set(sink.get() + 1))));
    assert!(child.is_alive(), "nothing but its parent owns a child scope");
    let value = child.run(|| create_signal(7).0);
    assert_eq!(value.get_untracked(), 7, "a child scope can be entered again");

    outer.dispose();
    assert_eq!(cleanups.get(), 1, "disposing the parent disposes the child");
    assert!(!child.is_alive());
    assert!(!value.is_alive(), "and frees what was made in it");
}
