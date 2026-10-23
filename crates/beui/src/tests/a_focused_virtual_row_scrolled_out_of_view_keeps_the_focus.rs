use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_focused_virtual_row_scrolled_out_of_view_keeps_the_focus() {
    let _root = crate::reactive::RootScope::new();
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <VirtualList
                        @node_ref=&list
                        keys={indices(VIRTUAL_ITEM_COUNT)}
                        item_size=VIRTUAL_ITEM_HEIGHT
                    >|key: ReadSignal<usize>| {
                        let label = create_memo(move || format!("Row {}", key.get()));
                        view! {
                            <Interactive focusable=true>
                                <Frame height=VIRTUAL_ITEM_HEIGHT>
                                    <Text>{label}</Text>
                                </Frame>
                            </Interactive>
                        }
                    }</VirtualList>
                </Offset>
            </List>
        }
    });
    let (scroll, list) = (scroll.get(), list.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    let focused = harness
        .document()
        .focused
        .expect("Tab focuses the first row");
    assert!(text_within(harness.document(), focused, "Row 0").is_some());

    let far = VIRTUAL_ITEM_COUNT as f32 * VIRTUAL_ITEM_HEIGHT / 2.0;
    harness.document_mut().set_scroll_offset(scroll, far);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(harness.document().focused, Some(focused));
    assert!(harness.document().children(list).contains(&focused));
    assert!(text_within(harness.document(), focused, "Row 0").is_some());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.settle();
    let next = harness
        .document()
        .focused
        .expect("Tab moves to the next row");
    assert!(
        text_within(harness.document(), next, "Row 1").is_some(),
        "Tab goes to the row after the focused one, not to the rows in view"
    );
    assert_eq!(
        harness.document().scroll_offset(scroll),
        VIRTUAL_ITEM_HEIGHT
    );
    assert!(
        harness
            .document()
            .children(list)
            .into_iter()
            .all(|row| text_within(harness.document(), row, "Row 0").is_none()),
        "the row the focus left is released once it is out of view"
    );
}
