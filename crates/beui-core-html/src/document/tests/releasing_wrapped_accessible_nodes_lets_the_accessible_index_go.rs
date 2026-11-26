use super::*;

#[test]
fn releasing_wrapped_accessible_nodes_lets_the_accessible_index_go() {
    let before = accessible_count();
    for _ in 0..100 {
        let scope = beui_reactive::Scope::root();
        scope.run(|| {
            let control = create("control", "button");
            let wrapper = wrap("wrapper", control);
            set_accessibility(wrapper, accesskit::Node::new(accesskit::Role::Button), 0);
        });
        scope.dispose();
        drop(scope);
    }
    create("after", "div");
    assert!(
        accessible_count() <= before + 1,
        "a wrapper and its control released together leave nothing in the index: {} from {before}",
        accessible_count()
    );
}
