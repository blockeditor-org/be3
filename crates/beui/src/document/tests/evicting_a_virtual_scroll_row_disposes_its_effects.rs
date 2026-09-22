use super::*;
use crate::reactive::{
    Frame, ItemSize, List, NodeRef, Offset, Text, VirtualList, build, create_signal, view,
};

#[test]
fn evicting_a_virtual_scroll_row_disposes_its_effects() {
    let (shown, set_shown) = create_signal(true);
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (scroll, list) = (scroll.clone(), list.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList @node_ref=&list keys={indices(100)} item_size=20.0>
                            {move |index: usize| {
                                let shown = shown.clone();
                                view! {
                                    <Frame visible={shown}>
                                        <Text string={format!("Row {index}")} />
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

    let mut harness = Harness::sized(document, Vec2::new(400.0, 300.0));
    harness.frame(Vec::new());
    let original = harness.document().children(list);
    assert!(!original.is_empty());

    harness.document.set_scroll_offset(scroll, f32::MAX);
    harness.frame(Vec::new());
    assert!(original.iter().all(|&id| !harness.document().contains(id)));

    with_installed(harness.document_mut(), |_| set_shown.set(false));
    harness.frame(Vec::new());
}
