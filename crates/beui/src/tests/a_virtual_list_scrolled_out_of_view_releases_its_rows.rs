use super::*;

const ROWS: usize = 10;
const BELOW: f32 = 1000.0;

#[test]
fn a_virtual_list_scrolled_out_of_view_releases_its_rows() {
    let built = Rc::new(RefCell::new(Vec::new()));
    let sink = built.clone();
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (scroll, list) = (scroll.clone(), list.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList
                            @node_ref=&list
                            keys={indices(ROWS)}
                            item_size=VIRTUAL_ITEM_HEIGHT
                        >
                            {move |key: usize| {
                                sink.borrow_mut().push(key);
                                view! {
                                    <Frame height=VIRTUAL_ITEM_HEIGHT>
                                        <Spacer />
                                    </Frame>
                                }
                            }}
                        </VirtualList>
                        <Frame height=BELOW>
                            <Spacer />
                        </Frame>
                    </Offset>
                </List>
            }
        }
    });
    let (scroll, list) = (scroll.get(), list.get());

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let rows = harness.document.children(list);
    assert_eq!(rows.len(), ROWS);

    built.borrow_mut().clear();
    harness.document.set_scroll_offset(scroll, BELOW / 2.0);
    harness.frame(Vec::new());
    assert!(harness.document.children(list).is_empty());
    assert!(rows.iter().all(|&row| !harness.document.contains(row)));

    harness.document.set_scroll_offset(scroll, 0.0);
    harness.frame(Vec::new());
    assert_eq!(*built.borrow(), indices(ROWS));
}
