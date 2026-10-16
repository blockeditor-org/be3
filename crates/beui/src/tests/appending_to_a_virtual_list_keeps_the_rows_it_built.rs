use super::*;

#[test]
fn appending_to_a_virtual_list_keeps_the_rows_it_built() {
    let _root = crate::reactive::RootScope::new();
    let (built, set_built) = create_signal(Vec::new());
    let (keys, set_keys) = create_signal(indices(VIRTUAL_ITEM_COUNT));
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build({
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList @node_ref=&list keys={keys} item_size=VIRTUAL_ITEM_HEIGHT>
                            |index: ReadSignal<usize>| {
                                create_binding(move || {
                                    let index = index.get();
                                    set_built.update(|built| built.push(index));
                                });
                                view! {
                                    <Frame height=VIRTUAL_ITEM_HEIGHT>
                                        <Spacer />
                                    </Frame>
                                }
                            }
                        </VirtualList>
                    </Offset>
                </List>
            }
        }
    });
    let (scroll, list) = (scroll.get(), list.get());

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let rows = harness.document.children(list);

    set_built.update(|built| built.clear());
    with_installed(harness.document_mut(), |_| {
        set_keys.set(indices(VIRTUAL_ITEM_COUNT + 1))
    });
    harness.frame(Vec::new());
    assert!(built.with_untracked(|built| built.is_empty()));
    assert_eq!(harness.document.children(list), rows);

    harness.document.set_scroll_offset(scroll, f32::MAX);
    harness.frame(Vec::new());
    assert_eq!(built.get_untracked().last(), Some(&VIRTUAL_ITEM_COUNT));
    let last = *harness.document.children(list).last().unwrap();
    assert_eq!(harness.rect(last).bottom(), VIEWPORT.y);
}
