use super::*;
use crate::reactive::{NodeRef, Portal, Show, create_signal, in_new_scope, with_reactive_scope};

#[test]
fn a_portal_shows_a_subtree_it_does_not_own() {
    let held = NodeRef::new();
    let (first, set_first) = create_signal(None);
    let (second, set_second) = create_signal(None);
    let (kept, set_kept) = create_signal(true);
    let document = {
        let held = held.clone();
        build(move || {
            let node = in_new_scope(|| {
                view! {
                    <Frame @node_ref=&held @test_id="held" height=20.0 />
                }
            });
            set_first.set(Some(node));
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

    with_reactive_scope(harness.document_mut(), move || set_second.set(Some(held)));
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(held).top(),
        harness.rect(harness.find("second")).top(),
        "the portal that takes the subtree is the one that lays it out"
    );

    with_reactive_scope(harness.document_mut(), move || set_kept.set(false));
    harness.frame(Vec::new());

    assert_eq!(
        harness.find("held"),
        held,
        "the subtree outlives the portal that was showing it"
    );
    assert!(
        harness.document().node_rect(held).is_none(),
        "a subtree no portal holds is not laid out"
    );
}
