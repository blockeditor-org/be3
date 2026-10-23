use super::*;
use crate::FontId;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn scrolling_a_nested_scroll_leaves_the_one_around_it_alone() {
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @test_id="outer">
                    <Frame height=150.0>
                        <Scroll @test_id="inner">
                            <Rows count=20 />
                        </Scroll>
                    </Frame>
                    <Rows count=20 />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (inner, outer) = (harness.find("inner"), harness.find("outer"));

    harness.scroll_at(pos2(200.0, 75.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(harness.document().scroll_offset(inner), 20.0);
    assert_eq!(harness.document().scroll_offset(outer), 0.0);

    harness.scroll_at(pos2(200.0, 250.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(harness.document().scroll_offset(inner), 20.0);
    assert_eq!(harness.document().scroll_offset(outer), 20.0);
}

#[component]
fn Rows(count: usize) -> DynamicSegment<ListChild> {
    view! {
        <ForEach keys={indices(count)}>|index: usize|
            <Text color=Color32::WHITE font={FontId::proportional(20.0)}>
                {format!("Row {index}")}
            </Text>
        </ForEach>
    }
}
