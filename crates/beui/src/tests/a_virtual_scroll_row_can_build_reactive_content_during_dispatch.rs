use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_virtual_scroll_row_can_build_reactive_content_during_dispatch() {
    let _root = crate::reactive::RootScope::new();
    let list = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0)>
                    <VirtualList
                        @node_ref=&list
                        keys={indices(VIRTUAL_ITEM_COUNT)}
                        item_size=VIRTUAL_ITEM_HEIGHT
                    >|index: ReadSignal<usize>|
                        <Text>{create_memo(move || format!("Row {}", index.get()))}</Text>
                    </VirtualList>
                </Offset>
            </List>
        }
    });
    let list = list.get();

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let first_row = harness.document().children(list)[0];
    assert_eq!(text_of(harness.document(), first_row), "Row 0");
}
