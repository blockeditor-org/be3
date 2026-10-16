use super::*;
use crate::reactive::*;

#[test]
fn compacting_virtual_rows_clamps_the_scroll_anchor_at_the_end() {
    let _root = crate::reactive::RootScope::new();
    let (keys, set_keys) = create_signal(indices(VIRTUAL_ITEM_COUNT));
    let (item_size, set_item_size) = create_signal(VIRTUAL_ITEM_HEIGHT);
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build({
        move || {
            let padding = create_memo(move || item_size.get() / 2.0);
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList
                            @node_ref=&list
                            keys={keys}
                            item_size={item_size}
                        >|_: ReadSignal<usize>|
                            <Frame
                                padding={create_memo(move || Sides::symmetric(0.0, padding.get()))}
                            >
                                <Spacer />
                            </Frame>
                        </VirtualList>
                    </Offset>
                </List>
            }
        }
    });
    let (scroll, list) = (scroll.get(), list.get());

    let mut harness = Harness::new(document);
    harness.document.set_scroll_offset(scroll, f32::MAX);
    harness.frame(Vec::new());

    let compact = VIRTUAL_ITEM_HEIGHT / 2.0;
    with_installed(harness.document_mut(), |_| set_item_size.set(compact));
    harness.frame(Vec::new());

    assert_eq!(
        harness.document.scroll_offset(scroll),
        VIRTUAL_ITEM_COUNT as f32 * compact - VIEWPORT.y
    );
    let children = harness.document.children(list);
    assert_eq!(harness.rect(*children.last().unwrap()).bottom(), VIEWPORT.y);

    with_installed(harness.document_mut(), |_| set_keys.set(Vec::new()));
    harness.frame(Vec::new());
    assert!(harness.document.children(list).is_empty());
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
