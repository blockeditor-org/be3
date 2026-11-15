use super::*;
use crate::create_effect_on_change;

#[test]
fn an_effect_on_change_runs_only_after_its_dependencies_change() {
    let _root = RootScope::new();
    let asked = create_signal(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let recorded = seen.clone();
    create_effect_on_change(
        move || asked.get(),
        move |asked, previous| recorded.borrow_mut().push((*asked, *previous)),
    );
    asked.set(1);
    flush();
    assert_eq!(
        *seen.borrow(),
        [(1, 0)],
        "a change before the first flush still runs it, and nothing runs for the value it started with"
    );
    asked.set(3);
    flush();
    assert_eq!(*seen.borrow(), [(1, 0), (3, 1)]);
}
