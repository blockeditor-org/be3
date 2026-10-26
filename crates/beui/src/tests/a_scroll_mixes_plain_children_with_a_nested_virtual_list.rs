use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

const HEADER: f32 = 50.0;
const FOOTER: f32 = 30.0;
const ROWS: usize = 500;

#[test]
fn a_scroll_mixes_plain_children_with_a_nested_virtual_list() {
    let _root = crate::reactive::RootScope::new();
    let built = create_signal(Vec::new());
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <List spacing=0.0>
                        <Frame height=HEADER>
                            <Spacer />
                        </Frame>
                        <List spacing=0.0>
                            <VirtualList
                                @node_ref=&list
                                keys={indices(ROWS)}
                                item_size=VIRTUAL_ITEM_HEIGHT
                            >|index: ReadSignal<usize>| {
                                create_binding(move || {
                                    let index = index.get();
                                    built.update(|built| built.push(index));
                                });
                                view! {
                                    <Frame height=VIRTUAL_ITEM_HEIGHT>
                                        <Spacer />
                                    </Frame>
                                }
                            }</VirtualList>
                        </List>
                        <Frame height=FOOTER>
                            <Spacer />
                        </Frame>
                    </List>
                </Scroll>
            </List>
        }
    });
    let (scroll, list) = (scroll.get(), list.get());

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rows = ROWS as f32 * VIRTUAL_ITEM_HEIGHT;
    let visible = ((VIEWPORT.y - HEADER) / VIRTUAL_ITEM_HEIGHT).ceil() as usize;
    assert_eq!(built.get_untracked(), (0..visible).collect::<Vec<usize>>());
    assert_eq!(harness.rect(list).top(), HEADER);
    assert_eq!(harness.rect(list).height(), rows);
    assert_eq!(
        harness.rect(harness.document().children(list)[0]).top(),
        HEADER
    );

    harness.document_mut().set_scroll_offset(scroll, f32::MAX);
    harness.frame(Vec::new());

    let content = HEADER + rows + FOOTER;
    assert_eq!(
        harness.document().scroll_offset(scroll),
        content - VIEWPORT.y
    );
    let last = *harness.document().children(list).last().unwrap();
    assert_eq!(harness.rect(last).bottom(), VIEWPORT.y - FOOTER);
    assert_eq!(built.get_untracked().last(), Some(&(ROWS - 1)));
}
