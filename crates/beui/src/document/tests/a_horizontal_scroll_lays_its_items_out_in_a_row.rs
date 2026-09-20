use super::*;
use crate::base::Direction;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, Offset, build, view};

#[test]
fn a_horizontal_scroll_lays_its_items_out_in_a_row() {
    let first = NodeRef::new();
    let second = NodeRef::new();
    let (first_ref, second_ref) = (first.clone(), second.clone());
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Offset
                    @sizing=ItemSize::Percent(100.0)
                    @test_id="strip"
                    direction=Direction::Horizontal
                    offset=30.0
                >
                    <Frame @node_ref=&first_ref width=120.0 />
                    <Frame @node_ref=&second_ref width=120.0 />
                    <ForEach keys={indices(4)}>
                        {|_: usize| view! {
                            <Frame width=120.0 />
                        }}
                    </ForEach>
                </Offset>
            </List>
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
