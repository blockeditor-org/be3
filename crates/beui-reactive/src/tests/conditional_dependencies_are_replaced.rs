use super::*;

#[test]
fn conditional_dependencies_are_replaced() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let choose_left = create_signal(true);
    let left = create_signal(1);
    let right = create_signal(10);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || {
            seen.borrow_mut().push(if choose_left.get() {
                left.get()
            } else {
                right.get()
            })
        });
    });
    right.set(11);
    flush();
    choose_left.set(false);
    flush();
    left.set(2);
    flush();
    right.set(12);
    flush();
    assert_eq!(*seen.borrow(), [1, 11, 12]);
}
