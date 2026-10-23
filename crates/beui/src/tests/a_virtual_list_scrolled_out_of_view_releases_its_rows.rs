use super::*;

const ROWS: usize = 10;
const BELOW: f32 = 1000.0;

#[test]
fn a_virtual_list_scrolled_out_of_view_releases_its_rows() {
    let _root = crate::reactive::RootScope::new();
    let built = create_signal(Vec::new());
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <List spacing=0.0>
                        <VirtualList
                            @node_ref=&list
                            keys={indices(ROWS)}
                            item_size=VIRTUAL_ITEM_HEIGHT
                        >|key: ReadSignal<usize>| {
                            create_binding(move || {
                                let key = key.get();
                                built.update(|built| built.push(key));
                            });
                            view! {
                                <Frame height=VIRTUAL_ITEM_HEIGHT>
                                    <Spacer />
                                </Frame>
                            }
                        }</VirtualList>
                        <Frame height=BELOW>
                            <Spacer />
                        </Frame>
                    </List>
                </Offset>
            </List>
        }
    });
    let (scroll, list) = (scroll.get(), list.get());

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let rows = harness.document().children(list);
    assert_eq!(rows.len(), ROWS);

    built.update(|built| built.clear());
    harness
        .document_mut()
        .set_scroll_offset(scroll, BELOW / 2.0);
    harness.frame(Vec::new());
    assert!(harness.document().children(list).is_empty());

    harness.document_mut().set_scroll_offset(scroll, 0.0);
    harness.frame(Vec::new());
    assert!(built.with_untracked(|built| built.is_empty()));
    assert_eq!(harness.document().children(list), rows);
}
