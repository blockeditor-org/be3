use super::*;
use crate::base::Direction;
use crate::reactive::{ItemSize, NodeRef, Scroll, build, intrinsic, view};

#[test]
fn a_horizontal_scroll_lays_its_items_out_in_a_row() {
    let first = NodeRef::new();
    let second = NodeRef::new();
    let (first_ref, second_ref) = (first.clone(), second.clone());
    let document = build(move || {
        let mut items = vec![
            intrinsic(view! {
                <Frame @node_ref=&first_ref width=120.0 />
            }),
            intrinsic(view! {
                <Frame @node_ref=&second_ref width=120.0 />
            }),
        ];
        items.extend((0..4).map(|_| {
            intrinsic(view! {
                <Frame width=120.0 />
            })
        }));
        view! {
            <Column spacing=0.0>
                <Scroll
                    @sizing=ItemSize::Percent(100.0)
                    @test_id="strip"
                    direction=Direction::Horizontal
                    offset=30.0
                    children={items}
                />
            </Column>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(first.get()),
        Rect::from_min_size(pos2(-30.0, 0.0), Vec2::new(120.0, VIEWPORT.y))
    );
    assert_eq!(
        harness.rect(second.get()),
        Rect::from_min_size(pos2(90.0, 0.0), Vec2::new(120.0, VIEWPORT.y))
    );
}
