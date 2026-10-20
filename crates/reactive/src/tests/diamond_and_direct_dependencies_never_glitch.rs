use super::*;

#[test]
fn diamond_and_direct_dependencies_never_glitch() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let value = create_signal(1);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let left = create_memo(move || value.get() * 2);
        let right = create_memo(move || value.get() * 3);
        let sum = create_memo(move || left.get() + right.get());
        let seen = seen.clone();
        create_effect(move || seen.borrow_mut().push((value.get(), sum.get())));
    });
    value.set(2);
    value.set(3);
    assert_eq!(*seen.borrow(), [(1, 5), (2, 10), (3, 15)]);
}
