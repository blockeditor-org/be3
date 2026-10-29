use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn tab_builds_the_virtual_rows_beyond_the_ones_in_view() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <VirtualList
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
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let shown = (VIEWPORT.y / VIRTUAL_ITEM_HEIGHT) as usize;
    let rows = shown * 2;
    for row in 0..rows {
        harness.key(Key::Tab, Modifiers::NONE);
        let focused = harness.document().focused.expect("Tab reaches every row");
        assert!(
            text_within(harness.document(), focused, &format!("Row {row}")).is_some(),
            "Tab number {} lands on row {row}",
            row + 1
        );
    }
    harness.settle();
    assert_eq!(
        harness.document().scroll_offset(scroll),
        rows as f32 * VIRTUAL_ITEM_HEIGHT - VIEWPORT.y
    );

    harness.document_mut().update_focus(None);
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.settle();
    let last = harness
        .document()
        .focused
        .expect("Shift+Tab reaches the last row");
    assert!(
        text_within(
            harness.document(),
            last,
            &format!("Row {}", VIRTUAL_ITEM_COUNT - 1)
        )
        .is_some()
    );
    assert_eq!(
        harness.document().scroll_offset(scroll),
        VIRTUAL_ITEM_COUNT as f32 * VIRTUAL_ITEM_HEIGHT - VIEWPORT.y
    );
}
