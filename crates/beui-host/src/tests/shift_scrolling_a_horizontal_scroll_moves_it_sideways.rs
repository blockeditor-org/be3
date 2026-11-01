use super::*;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;
use beui::Direction;

#[test]
fn shift_scrolling_a_horizontal_scroll_moves_it_sideways() {
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll
                    @sizing=ItemSize::Percent(100.0)
                    @test_id="strip"
                    direction=Direction::Horizontal
                >
                    <ForEach keys={indices(20)}>|index: usize|
                        <Frame width=120.0>
                            <Text color=Color32::WHITE>{format!("Card {index}")}</Text>
                        </Frame>
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let strip = harness.find("strip");

    harness.scroll_at(pos2(200.0, 150.0), vec2(0.0, -40.0), Modifiers::SHIFT);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(strip), 40.0);

    harness.scroll_at(pos2(200.0, 150.0), vec2(-40.0, 0.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(strip), 80.0);
}
