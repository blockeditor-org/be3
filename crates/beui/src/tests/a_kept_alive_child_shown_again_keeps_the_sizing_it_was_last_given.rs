use super::*;
use crate::primitives::Frame;
use crate::reactive::*;

#[test]
fn a_kept_alive_child_shown_again_keeps_the_sizing_it_was_last_given() {
    let _root = crate::reactive::RootScope::new();
    let child = NodeRef::new();
    let shown = create_signal(true);
    let height = create_signal(40.0_f32);
    let document = build_node(move || {
        let given = create_memo(move || ItemSize::Fixed(height.get()));
        view! {
            <List spacing=0.0>
                <ShowKeepAlive condition={shown}>
                    <Frame @node_ref=&child @sizing={given} />
                </ShowKeepAlive>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    let drawn = |harness: &Harness| {
        harness
            .document()
            .node_rect(child.get())
            .map(|rect| rect.height())
    };
    harness.frame(Vec::new());
    height.set(90.0);
    harness.frame(Vec::new());
    assert_eq!(
        drawn(&harness),
        Some(90.0),
        "a new sizing reaches the child"
    );

    shown.set(false);
    harness.frame(Vec::new());
    shown.set(true);
    harness.frame(Vec::new());
    assert_eq!(
        drawn(&harness),
        Some(90.0),
        "shown again, it keeps the sizing it was last given, not the one it was built with"
    );
}
