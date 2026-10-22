use super::*;
use crate::FontId;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn keys_an_inner_scroll_cannot_use_scroll_the_one_around_it() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @test_id="outer">
                    <Frame height=100.0>
                        <Scroll @test_id="inner">
                            <Rows count=20 />
                        </Scroll>
                    </Frame>
                    <Rows count=20 />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(300.0, 200.0));
    harness.frame(Vec::new());
    let (inner, outer) = (harness.find("inner"), harness.find("outer"));
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(harness.document().focus_is_within(inner));

    harness.key(Key::End, Modifiers::NONE);
    let end = harness.document().scroll_offset(inner);
    assert!(end > 0.0);
    assert_eq!(harness.document().scroll_offset(outer), 0.0);

    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(harness.document().scroll_offset(inner), end);
    assert_eq!(harness.document().scroll_offset(outer), 40.0);

    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(harness.document().scroll_offset(inner), end);
    assert!(harness.document().scroll_offset(outer) > 40.0);

    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(harness.document().scroll_offset(inner), 0.0);
    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(harness.document().scroll_offset(inner), 0.0);
    assert_eq!(harness.document().scroll_offset(outer), 0.0);
    assert!(harness.document().focus_is_within(inner));
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
