use super::*;

#[test]
fn selector_reads_are_current_before_a_flush() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let selected = create_signal(0usize);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let selector = scope.run(|| create_selector(move || selected.get()));
    scope.run({
        let seen = seen.clone();
        move || create_effect(move || seen.borrow_mut().push(selector.is_selected(&2)))
    });
    assert_eq!(*seen.borrow(), [false]);
    selected.set(1);
    assert!(!selector.is_selected(&2));
    selected.set(2);
    assert!(selector.is_selected(&2));
    assert_eq!(seen.borrow().len(), 1);
    flush();
    assert_eq!(*seen.borrow(), [false, true]);
}
