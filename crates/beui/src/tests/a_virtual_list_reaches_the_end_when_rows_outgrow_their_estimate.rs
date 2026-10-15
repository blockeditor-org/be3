use super::*;
use crate::primitives::*;
use crate::reactive::*;

const ROWS: usize = 100;
const ESTIMATE: f32 = 20.0;
const ACTUAL: f32 = 40.0;
const SETTLING_FRAMES: usize = 8;

#[test]
fn a_virtual_list_reaches_the_end_when_rows_outgrow_their_estimate() {
    let _root = crate::reactive::RootScope::new();
    let (built, set_built) = create_signal(Vec::new());
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let document = build({
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList @node_ref=&list keys={indices(ROWS)} item_size=ESTIMATE>
                            {move |index: ReadSignal<usize>| {
                                create_binding(move || {
                                    let index = index.get();
                                    set_built.update(|built| built.push(index));
                                });
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
    assert_eq!(
        harness.rect(harness.document.children(list)[1]).top(),
        ACTUAL
    );

    for _ in 0..SETTLING_FRAMES {
        harness.document.set_scroll_offset(scroll, f32::MAX);
        harness.frame(Vec::new());
    }

    assert_eq!(built.get_untracked().last(), Some(&(ROWS - 1)));
    let last = *harness.document.children(list).last().unwrap();
    assert_eq!(harness.rect(last).bottom(), VIEWPORT.y);
    assert_eq!(
        harness.document.scroll_offset(scroll),
        harness.rect(list).height() - VIEWPORT.y
    );
}
