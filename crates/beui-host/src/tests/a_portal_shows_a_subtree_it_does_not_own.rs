use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_portal_shows_a_subtree_it_does_not_own() {
    let _root = crate::reactive::RootScope::new();
    let held = NodeRef::new();
    let first = create_signal(None);
    let second = create_signal(None);
    let kept = create_signal(true);
    let document = {
        build_node(move || {
            let node = view! {
                <Frame @node_ref=&held @test_id="held" height=20.0 />
            };
            first.set(Some(node));
            view! {
                <List spacing=0.0>
                    <Frame @test_id="first" height=40.0>
                        <Portal node={first} />
                    </Frame>
                    <Show condition={kept}>
                        <Frame @test_id="second" height=40.0>
                            <Portal node={second} />
                        </Frame>
                    </Show>
                </List>
            }
        })
    };
    let held = held.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(held).top(),
        harness.rect(harness.find("first")).top(),
        "a portal lays the subtree it was given out where it stands"
    );

    second.set(Some(held));
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(held).top(),
        harness.rect(harness.find("second")).top(),
        "the portal that takes the subtree is the one that lays it out"
    );

    kept.set(false);
    harness.frame(Vec::new());

    assert!(
        harness.document().contains(held),
        "the subtree outlives the portal that was showing it"
    );
    assert_eq!(
        harness.document().find_test_id("held"),
        None,
        "a subtree no portal holds is not in the tree"
    );
    assert!(
        harness.document().node_rect(held).is_none(),
        "a subtree no portal holds is not laid out"
    );
}
