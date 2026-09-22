use super::*;
use crate::reactive::{ItemSize, List, NodeRef, Offset, Spacer, VirtualList, build, view};

const ROWS: usize = 200;
const ESTIMATE: f32 = 20.0;
const ACTUAL: f32 = 45.0;
const STEP: f32 = 90.0;
const STEPS: usize = 12;

#[test]
fn scrolling_back_up_a_virtual_list_keeps_its_rows_adjacent() {
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (scroll, list) = (scroll.clone(), list.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList @node_ref=&list keys={indices(ROWS)} item_size=ESTIMATE>
                            {move |_: usize| {
                                view! {
                                    <Frame height=ACTUAL>
                                        <Spacer />
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

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    for _ in 0..STEPS {
        let offset = harness.document.scroll_offset(scroll);
        harness.document.set_scroll_offset(scroll, offset + STEP);
        harness.frame(Vec::new());
        check_rows(&harness, list);
    }
    for _ in 0..STEPS {
        let offset = harness.document.scroll_offset(scroll);
        harness.document.set_scroll_offset(scroll, offset - STEP);
        harness.frame(Vec::new());
        check_rows(&harness, list);
    }

    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
    assert_eq!(harness.rect(harness.document.children(list)[0]).top(), 0.0);
}

fn check_rows(harness: &Harness, list: NodeId) {
    let rows = harness.document().children(list);
    assert!(!rows.is_empty());
    for pair in rows.windows(2) {
        assert_eq!(
            harness.rect(pair[0]).bottom(),
            harness.rect(pair[1]).top(),
            "rows drifted apart"
        );
    }
    assert!(
        harness.rect(rows[0]).top() <= 0.0,
        "a gap opened at the top"
    );
    assert!(
        harness.rect(*rows.last().unwrap()).bottom() >= VIEWPORT.y,
        "a gap opened at the bottom"
    );
}
