use std::cell::Cell;
use std::rc::Rc;

use super::*;

#[derive(Clone)]
struct Echo {
    source: Signal<u32>,
    seen: Rc<Cell<u32>>,
}

#[test]
fn a_tree_local_outlives_and_ignores_the_scope_that_first_asked_for_it() {
    let first = ::reactive::Scope::detached().pausable();
    first.pause();
    let made = first.run(|| {
        tree_local(|| {
            let source = create_signal(0);
            let seen = Rc::new(Cell::new(0));
            let echoed = seen.clone();
            create_effect(move || echoed.set(source.get()));
            Echo { source, seen }
        })
    });
    first.dispose();
    ::reactive::flush();

    let later: Echo = tree_local(|| unreachable!("the first value is kept"));
    assert!(Rc::ptr_eq(&made.seen, &later.seen));
    assert!(later.source.is_alive());
    later.source.set(5);
    ::reactive::flush();
    assert_eq!(later.seen.get(), 5);
}
