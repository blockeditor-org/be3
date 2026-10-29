use super::*;
use crate::reactive::*;

#[test]
fn a_node_whose_scope_is_disposed_leaves_the_parent_that_held_it() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal(None);
    let list = NodeRef::new();
    let frame = NodeRef::new();
    let document = build_node(move || {
        let scope = Scope::new();
        let (listed, framed) = scope.run(|| {
            (
                view! {
                    <Frame width=10.0 height=10.0 />
                },
                view! {
                    <Frame width=10.0 height=10.0 />
                },
            )
        });
        held.update(|held| *held = Some((scope, listed, framed)));
        view! {
            <List @node_ref=&list spacing=0.0>
                {listed}
                <Frame width=20.0 height=20.0 />
                <Frame @node_ref=&frame>{framed}</Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (scope, listed, framed) = held.update(|held| held.take().expect("the scope was built"));
    assert!(harness.document().children(list.get()).contains(&listed));
    assert_eq!(harness.document().children(frame.get()), vec![framed]);

    scope.dispose();
    harness.frame(Vec::new());
    assert!(!harness.document().contains(listed));
    assert!(
        !harness.document().children(list.get()).contains(&listed),
        "removing a node takes it out of the list that held it"
    );
    assert!(
        harness.document().children(frame.get()).is_empty(),
        "and out of the frame that held it"
    );
    assert_eq!(
        harness.document().children(list.get()).len(),
        2,
        "the children around it stay"
    );
    harness.frame(vec![Input::PointerMoved(pos2(5.0, 5.0))]);
}
