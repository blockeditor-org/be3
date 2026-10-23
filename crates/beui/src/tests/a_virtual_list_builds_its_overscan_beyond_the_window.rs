use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_virtual_list_builds_its_overscan_beyond_the_window() {
    let _root = crate::reactive::RootScope::new();
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <VirtualList
                        @node_ref=&list
                        keys={indices(100)}
                        item_size=VIRTUAL_ITEM_HEIGHT
                        overscan={VIRTUAL_ITEM_HEIGHT * 2.0}
                    >|_: ReadSignal<usize>|
                        <Frame height=VIRTUAL_ITEM_HEIGHT>
                            <Spacer />
                        </Frame>
                    </VirtualList>
                </Offset>
            </List>
        }
    });
    let (scroll, list) = (scroll.get(), list.get());

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let shown = (VIEWPORT.y / VIRTUAL_ITEM_HEIGHT) as usize;
    assert_eq!(harness.document().children(list).len(), shown + 2);

    harness
        .document_mut()
        .set_scroll_offset(scroll, VIRTUAL_ITEM_HEIGHT * 10.0);
    harness.frame(Vec::new());
    assert_eq!(harness.document().children(list).len(), shown + 4);
}
