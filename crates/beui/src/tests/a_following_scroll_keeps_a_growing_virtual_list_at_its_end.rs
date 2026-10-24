use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

const ESTIMATE: f32 = 20.0;
const ACTUAL: f32 = 36.0;

#[test]
fn a_following_scroll_keeps_a_growing_virtual_list_at_its_end() {
    let _root = crate::reactive::RootScope::new();
    let keys = create_signal(indices(30));
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll follow_end=true>
                    <VirtualList
                        @node_ref=&list
                        keys={keys}
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
    let last_bottom = |harness: &Harness| {
        let last = *harness.document().children(list).last().unwrap();
        harness.rect(last).bottom()
    };
    assert_eq!(last_bottom(&harness), VIEWPORT.y);

    for count in [31, 35, 60, 200] {
        with_installed(harness.document_mut(), |_| keys.set(indices(count)));
        harness.frame(Vec::new());
        assert_eq!(last_bottom(&harness), VIEWPORT.y, "{count} rows");
        assert_eq!(
            harness.document().scroll_offset(scroll),
            harness.rect(list).height() - VIEWPORT.y
        );
    }

    let center = harness.center(scroll);
    harness.frame(vec![
        Input::PointerMoved(center),
        Input::Scroll(ScrollDelta::Pixels(Vec2::new(0.0, 100.0))),
    ]);
    harness.frame(Vec::new());
    let away = harness.document().scroll_offset(scroll);
    assert!(away < harness.rect(list).height() - VIEWPORT.y);
    let shown = harness.document().children(list)[0];
    let top = harness.rect(shown).top();

    with_installed(harness.document_mut(), |_| keys.set(indices(220)));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(shown).top(), top);
    assert!(last_bottom(&harness) > VIEWPORT.y);

    harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(Vec2::new(
        0.0, -100_000.0,
    )))]);
    harness.frame(Vec::new());
    with_installed(harness.document_mut(), |_| keys.set(indices(240)));
    harness.frame(Vec::new());
    assert_eq!(last_bottom(&harness), VIEWPORT.y);
}
