use super::*;
use crate::geometry::vec2;
use crate::reactive::{DynamicSegment, ForEach, ItemSize, List, Offset, component, view};
use crate::unstyled::Scroll;

#[test]
fn an_offset_leaves_the_wheel_to_the_scroll_around_it() {
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @test_id="scroll">
                    <Frame height=150.0>
                        <Offset @test_id="offset">
                            <Rows count=20 />
                        </Offset>
                    </Frame>
                    <Rows count=20 />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (offset, scroll) = (harness.find("offset"), harness.find("scroll"));

    harness.scroll(pos2(200.0, 75.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().scroll_offset(offset),
        0.0,
        "an offset answers no input of its own"
    );
    assert_eq!(
        harness.document().scroll_offset(scroll),
        20.0,
        "the wheel over an offset reaches the scroll around it"
    );
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
