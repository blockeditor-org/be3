use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::primitives::List;

#[test]
fn a_wheel_turned_before_the_pointer_moves_on_scrolls_what_it_was_over() {
    let _root = crate::reactive::RootScope::new();
    let (left, right) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(50.0)>
                    <List spacing=0.0>
                        <Frame @node_ref=&left height=400.0 />
                        <Frame height=400.0 />
                    </List>
                </Scroll>
                <Scroll @sizing=ItemSize::Percent(50.0)>
                    <List spacing=0.0>
                        <Frame @node_ref=&right height=400.0 />
                        <Frame height=400.0 />
                    </List>
                </Scroll>
            </List>
        }
    });
    let (left, right) = (left.get(), right.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (left_top, right_top) = (harness.rect(left).top(), harness.rect(right).top());

    harness.frame(vec![
        Input::PointerMoved(harness.center(left)),
        Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, -40.0))),
        Input::PointerMoved(harness.center(right)),
    ]);
    harness.frame(Vec::new());

    assert!(
        harness.rect(left).top() < left_top,
        "the scroll the pointer was over when the wheel turned moves"
    );
    assert_eq!(
        harness.rect(right).top(),
        right_top,
        "and the one it moved to afterwards does not"
    );
}
