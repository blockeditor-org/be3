use super::*;
use crate::{provide_context, use_context};

#[derive(Clone, PartialEq, Debug)]
struct Theme(u32);

#[test]
fn a_detached_scope_reads_context_without_its_parent_disposing_it() {
    let parent = Scope::new();
    let (detached, disposals) = parent.run(|| {
        provide_context(Theme(7));
        let scope = Scope::detached();
        let disposals = Rc::new(Cell::new(0));
        let seen = scope.run({
            let disposals = disposals.clone();
            move || {
                on_cleanup(move || disposals.set(disposals.get() + 1));
                use_context::<Theme>()
            }
        });
        assert_eq!(
            seen,
            Some(Theme(7)),
            "a detached scope still reads the context of the scope it was opened in"
        );
        (scope, disposals)
    });

    parent.dispose();
    assert_eq!(
        disposals.get(),
        0,
        "a detached scope is not disposed by the scope it was opened in"
    );
    assert!(!detached.is_disposed());

    drop(detached);
    assert_eq!(disposals.get(), 1, "its owner disposes it instead");
}
