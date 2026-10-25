use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{Scroll, ScrollApi};

const ESTIMATE: f32 = 20.0;
const ACTUAL: f32 = 44.0;

#[test]
fn scroll_to_end_reaches_the_end_of_a_virtual_list_in_one_frame() {
    let _root = crate::reactive::RootScope::new();
    let api = ComponentRef::<ScrollApi>::new();
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll api>
                    <VirtualList
                        @node_ref=&list
                        keys={indices(500)}
                        item_size=ESTIMATE
                    >|_index: ReadSignal<usize>|
                        <Frame height=ACTUAL>
                            <Spacer />
                        </Frame>
                    </VirtualList>
                </Scroll>
            </List>
        }
    });
    let (scroll, list) = (scroll.get(), list.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);

    with_installed(harness.document_mut(), |_| api.get().scroll_to_end());
    harness.frame(Vec::new());
    let last = *harness.document().children(list).last().unwrap();
    assert_eq!(harness.rect(last).bottom(), VIEWPORT.y);
    assert_eq!(
        harness.document().scroll_offset(scroll),
        harness.rect(list).height() - VIEWPORT.y
    );
}
