use super::*;
use crate::base::Direction;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[component]
fn Strip() -> NodeId {
    view! {
        <Scroll @test_id="strip" direction=Direction::Horizontal>
            <ForEach keys={indices(20)}>|index: usize|
                <Frame width=120.0 height=40.0>
                    <Text color=Color32::WHITE>{format!("Card {index}")}</Text>
                </Frame>
            </ForEach>
        </Scroll>
    }
}

#[test]
fn a_vertical_wheel_scrolls_a_horizontal_scroll_unless_something_around_it_scrolls_vertically() {
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Strip />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let strip = harness.find("strip");
    let inside = harness.center(strip);
    harness.scroll_at(inside, vec2(0.0, -40.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().scroll_offset(strip),
        40.0,
        "with nothing to scroll vertically, the wheel turns the strip sideways"
    );

    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @test_id="page" @sizing=ItemSize::Percent(100.0)>
                    <Strip />
                    <ForEach keys={indices(40)}>|index: usize|
                        <Frame height=40.0>
                            <Text color=Color32::WHITE>{format!("Row {index}")}</Text>
                        </Frame>
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let strip = harness.find("strip");
    let page = harness.find("page");
    let inside = harness.center(strip);
    harness.scroll_at(inside, vec2(0.0, -40.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(strip), 0.0);
    assert_eq!(
        harness.document().scroll_offset(page),
        40.0,
        "the page around the strip takes the vertical wheel"
    );
}
