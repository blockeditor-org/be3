use super::*;
use crate::primitives::*;
use crate::reactive::*;

const ROWS: usize = 4;
const ESTIMATE: f32 = 10.0;
const ACTUAL: f32 = 30.0;

#[test]
fn a_virtual_list_in_a_box_taller_than_its_rows_builds_every_row() {
    let _root = crate::reactive::RootScope::new();
    let list = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0)>
                    <Frame height={Extent::at_least(VIEWPORT.y)}>
                        <List spacing=0.0>
                            <VirtualList
                                @node_ref=&list
                                keys={indices(ROWS)}
                                item_size=ESTIMATE
                            >|_: ReadSignal<usize>|
                                <Frame height=ACTUAL>
                                    <Spacer />
                                </Frame>
                            </VirtualList>
                        </List>
                    </Frame>
                </Offset>
            </List>
        }
    });
    let list = list.get();

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().children(list).len(),
        ROWS,
        "rows that measure more than the estimate still leave room for the last one"
    );
    assert_eq!(harness.rect(list).height(), ACTUAL * ROWS as f32);
}
