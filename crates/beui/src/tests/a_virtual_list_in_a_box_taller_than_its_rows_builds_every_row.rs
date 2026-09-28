use super::*;
use crate::reactive::{ItemSize, List, NodeRef, Offset, Spacer, VirtualList, build, view};

const ROWS: usize = 4;
const ESTIMATE: f32 = 10.0;
const ACTUAL: f32 = 30.0;

#[test]
fn a_virtual_list_in_a_box_taller_than_its_rows_builds_every_row() {
    let list = NodeRef::new();
    let document = build({
        let list = list.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0)>
                        <Frame min_height={VIEWPORT.y}>
                            <List spacing=0.0>
                                <VirtualList
                                    @node_ref=&list
                                    keys={indices(ROWS)}
                                    item_size=ESTIMATE
                                >
                                    {move |_: usize| view! {
                                        <Frame height=ACTUAL>
                                            <Spacer />
                                        </Frame>
                                    }}
                                </VirtualList>
                            </List>
                        </Frame>
                    </Offset>
                </List>
            }
        }
    });
    let list = list.get();

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document.children(list).len(),
        ROWS,
        "rows that measure more than the estimate still leave room for the last one"
    );
    assert_eq!(harness.rect(list).height(), ACTUAL * ROWS as f32);
}
