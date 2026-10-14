use super::*;
use crate::{create_binding, provide_context, use_context};

#[test]
fn a_binding_reads_the_context_of_the_scope_it_runs_in() {
    let _root = RootScope::new();
    let seen = Rc::new(Cell::new(None));
    let scope = Scope::new();

    scope.run({
        let seen = seen.clone();
        move || {
            provide_context(5u32);
            create_binding(move || seen.set(use_context::<u32>()));
        }
    });

    assert_eq!(seen.get(), Some(5));
}
