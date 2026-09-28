use super::*;
use crate::reactive::{Direction, Frame, List, build, view};

#[test]
fn a_fragment_sits_beside_the_children_written_around_it() {
    let (first, second, third, fourth) = (
        NodeRef::new(),
        NodeRef::new(),
        NodeRef::new(),
        NodeRef::new(),
    );
    let document = build({
        let (first, second, third, fourth) =
            (first.clone(), second.clone(), third.clone(), fourth.clone());
        move || {
            let middle = view! {
                <Frame @node_ref=&second width=20.0 />
                <Frame @node_ref=&third width=30.0 />
            };
            view! {
                <List direction=Direction::Horizontal spacing=0.0>
                    <Frame @node_ref=&first width=10.0 />
                    {middle}
                    <Frame @node_ref=&fourth width=40.0 />
                </List>
            }
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let lefts: Vec<f32> = [first, second, third, fourth]
        .iter()
        .map(|node| harness.rect(node.get()).left())
        .collect();
    assert_eq!(
        lefts,
        vec![0.0, 10.0, 30.0, 60.0],
        "a fragment written as one child must take the places between the children around it"
    );
}
