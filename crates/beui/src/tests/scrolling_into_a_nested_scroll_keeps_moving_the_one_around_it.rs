use super::*;
use crate::geometry::vec2;
use crate::interact::WHEEL_LATCH_TIMEOUT;
use crate::reactive::{DynamicSegment, ForEach, ItemSize, List, component, view};
use crate::unstyled::Scroll;

#[test]
fn scrolling_into_a_nested_scroll_keeps_moving_the_one_around_it() {
    let mut harness = Harness::new(nested());
    harness.frame(Vec::new());
    let (inner, outer) = (harness.find("inner"), harness.find("outer"));
    let still = pos2(200.0, 190.0);

    harness.scroll(still, vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(outer), 20.0);
    assert!(harness.rect(inner).contains(still));

    harness.scroll(still, vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(inner), 0.0);
    assert_eq!(harness.document().scroll_offset(outer), 40.0);

    harness.advance(WHEEL_LATCH_TIMEOUT);
    harness.scroll(still, vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(inner), 20.0);
    assert_eq!(harness.document().scroll_offset(outer), 40.0);
}

#[test]
fn moving_the_pointer_ends_the_wheel_latch() {
    let mut harness = Harness::new(nested());
    harness.frame(Vec::new());
    let (inner, outer) = (harness.find("inner"), harness.find("outer"));

    harness.scroll(pos2(200.0, 190.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(outer), 20.0);

    harness.scroll(pos2(200.0, 250.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().scroll_offset(inner),
        20.0,
        "the wheel scrolls what the pointer moved onto, however soon after the last turn"
    );
    assert_eq!(harness.document().scroll_offset(outer), 20.0);
}

fn nested() -> Document {
    build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @test_id="outer">
                    <Frame height=200.0 />
                    <Frame height=150.0>
                        <Scroll @test_id="inner">
                            <Rows count=20 />
                        </Scroll>
                    </Frame>
                    <Rows count=20 />
                </Scroll>
            </List>
        }
    })
}

#[component]
fn Rows(count: usize) -> DynamicSegment<NodeId> {
    view! {
        <ForEach keys={indices(count)}>
            {|index: usize| view! {
                <Text string={format!("Row {index}")} font_size=20.0 color=Color32::WHITE />
            }}
        </ForEach>
    }
}
