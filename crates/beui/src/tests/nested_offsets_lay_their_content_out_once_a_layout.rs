use super::*;
use crate::reactive::{ItemSize, List, NodeRef, Offset, Spacer, VirtualList, build, view};

const ROWS: usize = 1000;
const ESTIMATE: f32 = 20.0;

fn height(index: usize) -> f32 {
    10.0 + (index % 7) as f32 * 9.0
}

#[test]
fn nested_offsets_lay_their_content_out_once_a_layout() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build({
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <Offset fit=true>
                            <Offset fit=true>
                                <Offset fit=true>
                                    <Offset fit=true>
                                        <Offset fit=true>
                                            <VirtualList keys={indices(ROWS)} item_size=ESTIMATE>
                                                {move |index: usize| view! {
                                                    <Frame height={height(index)}>
                                                        <Spacer />
                                                    </Frame>
                                                }}
                                            </VirtualList>
                                        </Offset>
                                    </Offset>
                                </Offset>
                            </Offset>
                        </Offset>
                    </Offset>
                </List>
            }
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.document.set_scroll_offset(scroll, 5000.0);
    harness.frame(Vec::new());

    let latest = harness.document().performance().latest;
    assert!(
        latest.work.placed <= latest.layout_passes * latest.nodes,
        "each layout lays each node out at most once: {} placements over {} layouts of {} nodes",
        latest.work.placed,
        latest.layout_passes,
        latest.nodes
    );
}
