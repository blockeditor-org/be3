use super::*;

#[test]
fn appending_to_a_virtual_list_keeps_the_rows_it_built() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let (count, set_count) = create_signal(VIRTUAL_ITEM_COUNT);
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (scroll, list, sink) = (scroll.clone(), list.clone(), built.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList @node_ref=&list count={count} item_size=VIRTUAL_ITEM_HEIGHT>
                            {move |index: usize| {
                                sink.borrow_mut().push(index);
                                view! {
                                    <Frame height=VIRTUAL_ITEM_HEIGHT>
                                        <Spacer />
                                    </Frame>
                                }
                            }}
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

    built.borrow_mut().clear();
    with_installed(harness.document_mut(), |_| {
        set_count.set(VIRTUAL_ITEM_COUNT + 1)
    });
    harness.frame(Vec::new());
    assert!(built.borrow().is_empty());
    assert_eq!(harness.document.children(list), rows);

    harness.document.set_scroll_offset(scroll, f32::MAX);
    harness.frame(Vec::new());
    assert_eq!(built.borrow().last(), Some(&VIRTUAL_ITEM_COUNT));
    let last = *harness.document.children(list).last().unwrap();
    assert_eq!(harness.rect(last).bottom(), VIEWPORT.y);
}
