use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn evicting_a_virtual_scroll_row_disposes_its_effects() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(true);
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <VirtualList
                        @node_ref=&list
                        keys={indices(100)}
                        item_size=20.0
                    >|index: ReadSignal<usize>|
                        <List spacing=0.0>
                            <Show condition={shown}>
                                <Text>{create_memo(move || format!("Row {}", index.get()))}</Text>
                            </Show>
                        </List>
                    </VirtualList>
                </Scroll>
            </List>
        }
    });
    let (scroll, list) = (scroll.get(), list.get());

    let mut harness = Harness::sized(document, Vec2::new(400.0, 300.0));
    harness.frame(Vec::new());
    let original = harness.document().children(list);
    assert!(!original.is_empty());

    harness.document_mut().set_scroll_offset(scroll, f32::MAX);
    harness.frame(Vec::new());
    let rows = harness.document().children(list);
    let last = harness.document().children(*rows.last().unwrap())[0];
    assert_eq!(text_of(harness.document(), last), "Row 99");

    with_installed(harness.document_mut(), |_| shown.set(false));
    harness.frame(Vec::new());
}
