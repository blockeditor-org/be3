use super::*;
use crate::primitives::{Frame, List, Replica};
use crate::reactive::*;

#[test]
fn a_node_lent_to_a_second_replica_is_drawn_by_that_one_alone() {
    let _root = RootScope::new();
    let red = Color32::from_rgb(200, 30, 30);
    let lent_later = create_signal(false);
    let document = build_node(move || {
        let item = view! { <Frame @test_id="item" width=40.0 height=20.0 fill=red /> };
        let first = view! { <Replica of={Some(item)} /> };
        view! {
            <List spacing=10.0>
                {item}
                <Frame @test_id="first" width=40.0 height=20.0>{first}</Frame>
                <Frame @test_id="second" width=40.0 height=20.0>
                    <Show value={lent_later}>
                        <Replica of={Some(item)} />
                    </Show>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let drawn = |harness: &mut Harness| -> Vec<Rect> {
        harness
            .frame(Vec::new())
            .main()
            .shapes()
            .iter()
            .filter_map(|shape| match shape {
                crate::painter::Shape::Rect { rect, color, .. } if *color == red => Some(*rect),
                _ => None,
            })
            .collect()
    };
    let at = |harness: &Harness, id: &str| {
        Rect::from_min_size(harness.rect(harness.find(id)).min, vec2(40.0, 20.0))
    };
    let (first, second) = (at(&harness, "first"), at(&harness, "second"));
    assert_eq!(drawn(&mut harness), vec![first], "lent, the item is drawn by its replica");
    lent_later.set(true);
    harness.settle();
    assert_eq!(
        drawn(&mut harness),
        vec![second],
        "lent to a second replica, it is drawn by that one alone"
    );
}
