use super::*;
use crate::base::Direction;
use crate::reactive::{ForEach, ItemSize, List, build, view};
use crate::unstyled::Scroll;

#[test]
fn the_right_arrow_scrolls_a_horizontal_scroll_the_focus_is_in() {
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll
                    @sizing=ItemSize::Percent(100.0)
                    @test_id="strip"
                    direction=Direction::Horizontal
                >
                    <ForEach keys={indices(40)}>
                        {|index: usize| view! {
                            <Frame width=120.0>
                                <LabelledButton
                                    label={format!("Card {index}")}
                                    on_click={move || {}}
                                />
                            </Frame>
                        }}
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let strip = harness.find("strip");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(strip), 40.0);

    harness.key(Key::End, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(harness.document().scroll_offset(strip) > 40.0);

    harness.key(Key::Home, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(strip), 0.0);
}
