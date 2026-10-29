use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn pointer_motion_only_reaches_what_lies_under_the_pointer() {
    let _root = crate::reactive::RootScope::new();
    let hovers = create_signal::<Vec<bool>>(Vec::new());
    let (top, bottom) = (NodeRef::new(), NodeRef::new());
    let mut document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Interactive
                    @node_ref=&top
                    on_hover_change={move |hovered: bool| hovers.update(|hovers| hovers.push(hovered))}
                >
                    <Frame height=100.0 />
                </Interactive>
                <Interactive @node_ref=&bottom on_click={|| {}}>
                    <Frame height=100.0 />
                </Interactive>
            </List>
        }
    });
    let (top, bottom) = (top.get(), bottom.get());
    let top_counts = counted_with_measures(&mut document, top);
    let bottom_counts = counted_with_measures(&mut document, bottom);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(vec![Input::PointerMoved(pos2(5.0, 50.0))]);
    top_counts.visits.set(0);
    bottom_counts.visits.set(0);

    for x in [10.0, 20.0, 30.0] {
        harness.frame(vec![Input::PointerMoved(pos2(x, 50.0))]);
    }
    assert!(
        top_counts.visits.get_untracked() >= 3,
        "the catcher under the pointer is hit tested on every move"
    );
    assert_eq!(
        bottom_counts.visits.get_untracked(),
        0,
        "a catcher away from the pointer is not visited"
    );
    assert_eq!(hovers.get_untracked(), vec![true]);

    harness.frame(vec![Input::PointerMoved(pos2(10.0, 500.0))]);
    assert_eq!(
        hovers.get_untracked(),
        vec![true, false],
        "the catcher the pointer left hears it leave"
    );
    top_counts.visits.set(0);
    harness.frame(vec![Input::PointerMoved(pos2(20.0, 500.0))]);
    assert_eq!(
        top_counts.visits.get_untracked(),
        0,
        "once the pointer has left it, it is no longer visited"
    );
}
