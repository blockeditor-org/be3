use super::*;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn scrolling_copies_what_the_scroll_showed() {
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=100.0 fill=Color32::WHITE radius=0.0 />
                <Frame height=150.0>
                    <Scroll @test_id="scroll">
                        <Bands count=20 />
                    </Scroll>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let scroll = harness.find("scroll");
    let bounds = harness.rect(scroll);
    harness.frame(vec![Input::PointerMoved(bounds.center())]);
    harness.frame(Vec::new());
    let output = harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, -25.0)))]);
    let main = output.main();
    let moved = main.moved.expect("the scroll is copied");
    assert_eq!(moved.by, vec2(0.0, -25.0));
    assert!(
        main.damage.rects().iter().all(|rect| rect.height() <= 25.0),
        "only the rows the copy exposes are repainted: {:?}",
        main.damage.rects()
    );
}

#[component]
fn Bands(count: usize) -> DynamicSegment<ListChild> {
    view! {
        <ForEach keys={indices(count)}>|index: usize|
            <Frame height=40.0 fill={Color32::from_gray((index * 10) as u8)} radius=0.0 />
        </ForEach>
    }
}
