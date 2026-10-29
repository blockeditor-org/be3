use super::*;
use crate::primitives::*;
use crate::reactive::*;

const ROWS: usize = 200;
const FOCUSABLE: usize = 150;

#[test]
fn tab_skips_virtual_rows_without_a_control() {
    let _root = crate::reactive::RootScope::new();
    let (scroll, list, target) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <VirtualList
                        @node_ref=&list
                        keys={indices(ROWS)}
                        item_size=VIRTUAL_ITEM_HEIGHT
                    >|key: ReadSignal<usize>|
                        <List spacing=0.0>
                            <Dynamic value={key}>|key: usize| {
                                match key == FOCUSABLE {
                                    true => view! {
                                        <Interactive focusable=true @node_ref=&target>
                                            <Frame height=VIRTUAL_ITEM_HEIGHT />
                                        </Interactive>
                                    },
                                    false => view! {
                                        <Frame height=VIRTUAL_ITEM_HEIGHT />
                                    },
                                }
                            }</Dynamic>
                        </List>
                    </VirtualList>
                </Offset>
            </List>
        }
    });
    let (scroll, list) = (scroll.get(), list.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(harness.document().focused, Some(target.get()));
    harness.settle();
    let shown = (VIEWPORT.y / VIRTUAL_ITEM_HEIGHT) as usize;
    assert_eq!(
        harness.document().scroll_offset(scroll),
        (FOCUSABLE + 1) as f32 * VIRTUAL_ITEM_HEIGHT - VIEWPORT.y
    );
    assert!(
        harness.document().children(list).len() <= shown + 1,
        "the rows Tab looked through and passed are released"
    );
}
