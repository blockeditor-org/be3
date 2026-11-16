use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_collapsed_frame_in_a_list_takes_no_space_or_spacing() {
    let _root = RootScope::new();
    let first = create_signal(false);
    let middle = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=10.0>
                <Frame collapsed={first}>
                    <Frame @test_id="a" height=20.0 />
                </Frame>
                <Frame collapsed={middle}>
                    <Frame @test_id="b" height=20.0>
                        <Text>"Folded away"</Text>
                    </Frame>
                </Frame>
                <Frame @test_id="c" height=20.0 />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    assert_eq!(top_of(&harness, "c"), 60.0);

    middle.set(true);
    harness.settle();
    assert_eq!(
        top_of(&harness, "c"),
        30.0,
        "a collapsed row takes neither its length nor the spacing before it"
    );
    assert!(
        harness.document().node_rect(harness.find("b")).is_none(),
        "and what it holds is not laid out, so it draws nothing"
    );
    let read = harness.accessible().into_iter().any(|node| {
        node.label()
            .is_some_and(|label| label.contains("Folded away"))
    });
    assert!(
        !read,
        "nor is it read out, as on the web, where it is display: none"
    );

    first.set(true);
    middle.set(false);
    harness.settle();
    assert_eq!(
        top_of(&harness, "b"),
        0.0,
        "the first row shown takes no spacing before it, whatever collapsed ahead of it"
    );
    assert_eq!(top_of(&harness, "c"), 30.0);
}
