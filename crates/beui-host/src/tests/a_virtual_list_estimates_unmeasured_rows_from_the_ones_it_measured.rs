use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_virtual_list_estimates_unmeasured_rows_from_the_ones_it_measured() {
    let _root = crate::reactive::RootScope::new();
    let list = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0)>
                    <List spacing=0.0>
                        <VirtualList
                            @node_ref=&list
                            keys={indices(VIRTUAL_ITEM_COUNT)}
                            item_size={VIRTUAL_ITEM_HEIGHT * 3.0}
                        >|_: ReadSignal<usize>|
                            <Frame height=VIRTUAL_ITEM_HEIGHT>
                                <Spacer />
                            </Frame>
                        </VirtualList>
                    </List>
                </Offset>
            </List>
        }
    });
    let list = list.get();

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(list).height(),
        VIRTUAL_ITEM_COUNT as f32 * VIRTUAL_ITEM_HEIGHT
    );
}
