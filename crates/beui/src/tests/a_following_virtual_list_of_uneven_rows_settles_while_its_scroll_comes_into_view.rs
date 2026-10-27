use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

const ESTIMATE: f32 = 20.0;
const LOG_HEIGHT: f32 = 200.0;

fn height(index: usize) -> f32 {
    16.0 + ((index * 37) % 9) as f32 * 11.0
}

#[test]
fn a_following_virtual_list_of_uneven_rows_settles_while_its_scroll_comes_into_view() {
    let _root = crate::reactive::RootScope::new();
    let keys = create_signal(indices(12));
    let (page, log, list) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&page>
                    <Frame height=280.0 />
                    <Frame height=LOG_HEIGHT>
                        <Scroll @node_ref=&log follow_end=true>
                            <Frame padding={Sides::symmetric(0.0, 8.0)}>
                                <VirtualList
                                    @node_ref=&list
                                    keys={keys}
                                    item_size=ESTIMATE
                                >|index: ReadSignal<usize>| {
                                    let tall = create_memo(move || height(index.get()));
                                    view! {
                                        <Frame height={tall}>
                                            <Spacer />
                                        </Frame>
                                    }
                                }</VirtualList>
                            </Frame>
                        </Scroll>
                    </Frame>
                    <Frame height=600.0 />
                </Scroll>
            </List>
        }
    });
    let (page, log, list) = (page.get(), log.get(), list.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    for _ in 0..12 {
        let offset = harness.document().scroll_offset(page) + 25.0;
        harness.document_mut().set_scroll_offset(page, offset);
        harness.frame(Vec::new());
        assert!(harness.document().performance().latest.layout_passes <= 2);
        with_installed(harness.document_mut(), |_| {
            keys.update(|keys| keys.push(keys.len()))
        });
        harness.frame(Vec::new());
        assert!(
            harness.document().performance().latest.layout_passes <= 2,
            "a pinned list measures its rows from the end, so a second layout settles it"
        );
    }
    let last = *harness.document().children(list).last().unwrap();
    assert_eq!(
        harness.rect(last).bottom(),
        harness.rect(log).bottom() - 8.0,
        "the last row sits at the end, above the padding"
    );
}
