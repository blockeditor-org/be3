use super::*;
use crate::primitives::*;
use crate::reactive::*;

const ROWS: usize = 1000;
const ESTIMATE: f32 = 20.0;

fn height(index: usize) -> f32 {
    10.0 + (index % 7) as f32 * 9.0
}

#[test]
fn a_virtual_list_scrolled_to_rows_of_unestimated_heights_fills_its_window_in_one_walk() {
    let _root = crate::reactive::RootScope::new();
    let built = create_signal(Vec::new());
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <VirtualList
                        @node_ref=&list
                        keys={indices(ROWS)}
                        item_size=ESTIMATE
                    >|index: ReadSignal<usize>| {
                        create_binding(move || {
                            index.get();
                            let walk = with_document(|document| {
                                document.laying_out().map(|_| document.layout_pass())
                            });
                            built.update(|built| built.push(walk));
                        });
                        view! {
                            <Frame height={create_memo(move || height(index.get()))}>
                                <Spacer />
                            </Frame>
                        }
                    }</VirtualList>
                </Offset>
            </List>
        }
    });
    let (scroll, list) = (scroll.get(), list.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    built.update(|built| built.clear());
    harness.document_mut().set_scroll_offset(scroll, 5000.0);
    harness.frame(Vec::new());

    let walks = built.get_untracked();
    assert!(!walks.is_empty(), "the new region builds rows");
    assert!(
        walks.iter().all(|walk| walk.is_some() && *walk == walks[0]),
        "every row is built while the first walk reaches it, got {walks:?}"
    );
    assert!(
        harness.document().performance().latest.layout_passes <= 2,
        "a second layout follows only from the estimated total changing"
    );
    let mut rects: Vec<Rect> = harness
        .document()
        .children(list)
        .into_iter()
        .map(|row| harness.rect(row))
        .collect();
    rects.sort_by(|a, b| a.top().total_cmp(&b.top()));
    assert!(
        rects[0].top() <= 0.0,
        "the rows reach the top, got {rects:?}"
    );
    assert!(
        rects.last().unwrap().bottom() >= VIEWPORT.y,
        "the rows reach the bottom, got {rects:?}"
    );
    for pair in rects.windows(2) {
        assert_eq!(pair[0].bottom(), pair[1].top(), "the rows leave no gap");
    }
}
