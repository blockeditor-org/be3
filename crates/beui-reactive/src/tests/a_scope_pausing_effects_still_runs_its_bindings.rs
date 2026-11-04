use super::*;
use crate::create_binding;

#[test]
fn a_scope_pausing_effects_still_runs_its_bindings() {
    let _root = RootScope::new();
    let count = create_signal(0);
    let bound = Rc::new(Cell::new(0));
    let effected = Rc::new(Cell::new(0));
    let scope = Scope::new().pausable_effects();
    scope.pause();
    scope.run({
        let bound = bound.clone();
        let effected = effected.clone();
        move || {
            create_binding(move || bound.set(count.get()));
            create_effect(move || effected.set(count.get() + 100));
        }
    });

    count.set(1);
    flush();
    assert_eq!(bound.get(), 1);
    assert_eq!(effected.get(), 0);

    scope.resume();
    flush();
    assert_eq!(effected.get(), 101);
}
