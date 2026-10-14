use super::*;
use crate::create_binding;

#[test]
fn a_binding_that_reads_nothing_leaves_nothing_to_run() {
    let _root = RootScope::new();
    let (count, set_count) = create_signal(0);
    let runs = Rc::new(Cell::new(0));
    let scope = Scope::new();

    scope.run({
        let runs = runs.clone();
        move || {
            create_effect(move || {
                count.get();
            });
            create_binding(move || runs.set(runs.get() + 1));
        }
    });
    set_count.set(1);

    assert_eq!(runs.get(), 1);
}
