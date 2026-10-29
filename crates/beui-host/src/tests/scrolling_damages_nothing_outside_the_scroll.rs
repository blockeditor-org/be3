use super::*;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn scrolling_damages_nothing_outside_the_scroll() {
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=100.0 fill=Color32::WHITE radius=0.0 />
                <Frame height=150.0>
                    <Scroll @test_id="scroll">
                        <Bands count=20 />
                    </Scroll>
                </Frame>
                <Frame height=100.0 fill=Color32::WHITE radius=0.0 />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let scroll = harness.find("scroll");
    let bounds = harness.rect(scroll);

    harness.frame(vec![Input::PointerMoved(bounds.center())]);
    let damage = harness
        .frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, -25.0)))])
        .main()
        .damage()
        .expect("scrolling repaints");

    assert_eq!(harness.document().scroll_offset(scroll), 25.0);
    assert!(
        bounds.intersect(damage) == damage,
        "bands straddling the edge of the scroll are clipped to it, \
         so scrolling damages only the scroll {bounds:?}: {damage:?}"
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
