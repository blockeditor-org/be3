use super::*;

#[derive(Clone)]
struct Label(&'static str);

#[test]
fn a_scope_opened_inside_an_effect_lives_until_its_handle_drops() {
    let _root = RootScope::new();
    let outer = Scope::new();
    let tick = create_signal(0);
    let cleanups = Rc::new(Cell::new(0));
    let opened: Rc<RefCell<Vec<Scope>>> = Rc::new(RefCell::new(Vec::new()));

    let sink = cleanups.clone();
    let kept = opened.clone();
    outer.run(move || {
        crate::provide_context(Label("outer"));
        create_effect(move || {
            tick.get();
            let scope = owner_scope()
                .expect("an effect always has an owner")
                .child()
                .expect("the owner is alive while the effect runs");
            let sink = sink.clone();
            let label = scope.run(move || {
                on_cleanup(move || sink.set(sink.get() + 1));
                crate::use_context::<Label>().map(|label| label.0)
            });
            assert_eq!(label, Some("outer"), "the scope reads its owner's context");
            kept.borrow_mut().push(scope);
        });
    });

    tick.set(1);
    flush();
    assert_eq!(opened.borrow().len(), 2, "the effect ran twice");
    assert_eq!(
        cleanups.get(),
        0,
        "a scope opened inside an effect must survive that effect re-running"
    );

    outer.dispose();
    assert_eq!(
        cleanups.get(),
        0,
        "the handle owns the scope, so its owner leaves it alone"
    );

    opened.borrow_mut().clear();
    assert_eq!(
        cleanups.get(),
        2,
        "dropping the handles disposes the scopes"
    );
}
