use super::*;
use crate::reactive::{
    Frame, Interactive, ItemSize, List, NodeRef, Offset, RecyclingVirtualList, Text, build,
    create_memo, create_signal, view,
};

#[test]
fn a_recycling_virtual_list_shows_new_keys_in_the_rows_that_left() {
    let _root = crate::reactive::RootScope::new();
    let (built, set_built) = create_signal(0usize);
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build({
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <RecyclingVirtualList
                            @node_ref=&list
                            keys={indices(VIRTUAL_ITEM_COUNT)}
                            item_size=VIRTUAL_ITEM_HEIGHT
                        >
                            {move |key: ReadSignal<usize>| {
                                set_built.update(|built| *built += 1);
                                let label = create_memo(move || format!("Row {}", key.get()));
                                view! {
                                    <Interactive focusable=true>
                                        <Frame height=VIRTUAL_ITEM_HEIGHT>
                                            <Text string={label} />
                                        </Frame>
                                    </Interactive>
                                }
                            }}
                        </RecyclingVirtualList>
                    </Offset>
                </List>
            }
        }
    });
    let (scroll, list) = (scroll.get(), list.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let shown = harness.document.children(list).len();

    for step in 1..=20 {
        harness
            .document
            .set_scroll_offset(scroll, step as f32 * VIRTUAL_ITEM_HEIGHT * shown as f32);
        harness.frame(Vec::new());
    }
    assert!(
        built.get_untracked() <= shown * 3,
        "scrolling twenty windows built {} rows for a window of {shown}",
        built.get_untracked()
    );
    let labels = labels(&harness);
    let top = 20 * shown;
    assert!(labels.contains(&format!("Row {top}")), "{labels:?}");
    assert!(!labels.contains(&"Row 0".to_owned()), "{labels:?}");

    harness.document.set_scroll_offset(scroll, 0.0);
    harness.frame(Vec::new());
    let focused = harness.document.children(list)[0];
    harness.document.focus_focusable(focused);
    harness.frame(Vec::new());
    assert_eq!(harness.document.focused, Some(focused));
    for step in 40..=50 {
        harness
            .document
            .set_scroll_offset(scroll, step as f32 * VIRTUAL_ITEM_HEIGHT * shown as f32);
        harness.frame(Vec::new());
        assert!(
            !harness.document.children(list).contains(&focused),
            "a row holding focus must not be shown for another key"
        );
    }
}

fn labels(harness: &Harness) -> Vec<String> {
    let view = harness.document().accessibility_view().unwrap();
    view.nodes()
        .filter_map(|node| node.value().map(str::to_owned))
        .collect()
}
