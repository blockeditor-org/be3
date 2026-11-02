use super::*;

#[test]
fn nested_effects_are_disposed_before_parent_reruns() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let parent = create_signal(1);
    let child = create_signal(10);
    let seen = Rc::new(RefCell::new(Vec::new()));
    scope.run(|| {
        let seen = seen.clone();
        create_effect(move || {
            let parent = parent.get();
            let seen = seen.clone();
            create_effect(move || seen.borrow_mut().push((parent, child.get())));
        });
    });
    child.set(20);
    parent.set(2);
    flush();
    child.set(30);
    flush();
    assert_eq!(*seen.borrow(), [(1, 10), (2, 20), (2, 30)]);
}
