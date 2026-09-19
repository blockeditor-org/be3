use super::*;
use crate::base::Direction;
use crate::geometry::vec2;
use crate::reactive::{ItemSize, List, Scroll, build, view};

#[test]
fn shift_scrolling_a_horizontal_scroll_moves_it_sideways() {
    let document = build(move || {
        let items = (0..20)
            .map(|index| {
                view! {
                    <Frame width=120.0>
                        <Text
                            string={format!("Card {index}")}
                            font_size=14.0
                            color=Color32::WHITE
                        />
                    </Frame>
                }
            })
            .collect::<Vec<_>>();
        view! {
            <List spacing=0.0>
                <Scroll
                    @sizing=ItemSize::Percent(100.0)
                    @test_id="strip"
                    direction=Direction::Horizontal
                    children={items}
                />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let strip = harness.find("strip");

    harness.scroll(pos2(200.0, 150.0), vec2(0.0, -40.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(strip), 0.0);

    harness.scroll(pos2(200.0, 150.0), vec2(0.0, -40.0), Modifiers::SHIFT);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(strip), 40.0);

    harness.scroll(pos2(200.0, 150.0), vec2(-40.0, 0.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(strip), 80.0);
}
