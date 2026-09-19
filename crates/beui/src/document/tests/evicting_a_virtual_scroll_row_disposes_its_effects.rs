use super::*;
use crate::reactive::{
    Frame, ItemSize, List, NodeRef, Text, VirtualList, build, create_signal, view,
};

#[test]
fn evicting_a_virtual_scroll_row_disposes_its_effects() {
    let (shown, set_shown) = create_signal(true);
    let scroll = NodeRef::new();
    let document = build({
        let scroll = scroll.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <VirtualList
                        @sizing=ItemSize::Percent(100.0)
                        @node_ref=&scroll
                        count=100
                        item_size=20.0
                    >
                        {move |index: usize| {
                            let shown = shown.clone();
                            view! {
                                <Frame visible={shown}>
                                    <Text string={format!("Row {index}")} />
                                </Frame>
                            }
                        }}
                    </VirtualList>
                </List>
            }
        }
    });
    let scroll = scroll.get();

    let mut harness = Harness::sized(document, Vec2::new(400.0, 300.0));
    harness.frame(Vec::new());
    assert!(!harness.document().children(scroll).is_empty());

    *harness.viewport_mut() = Vec2::new(400.0, 0.0);
    harness.frame(Vec::new());
    assert!(harness.document().children(scroll).is_empty());
    *harness.viewport_mut() = Vec2::new(400.0, 300.0);
    harness.frame(Vec::new());
    assert!(!harness.document().children(scroll).is_empty());

    with_installed(harness.document_mut(), |_| set_shown.set(false));
    harness.frame(Vec::new());
}
