use super::*;
use crate::reactive::{
    ItemSize, List, NodeRef, Offset, VirtualList, build, create_memo, create_signal, view,
};
use crate::styled::Switch;

#[test]
fn flipping_a_switch_can_replace_the_items_of_a_scroll() {
    for inset in [7.0, 17.0] {
        check_compact_rows(inset);
    }
}

fn check_compact_rows(inset: f32) {
    let built = Rc::new(RefCell::new(Vec::new()));
    let (switch, scroll, list) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (switch, scroll, list) = (switch.clone(), scroll.clone(), list.clone());
        let sink = built.clone();
        move || {
            let (compact, set_compact) = create_signal(false);
            let item_height = create_memo(move || {
                if compact.get() {
                    VIRTUAL_ITEM_HEIGHT / 2.0
                } else {
                    VIRTUAL_ITEM_HEIGHT
                }
            });
            let padding = create_memo({
                let item_height = item_height.clone();
                move || item_height.get() / 2.0
            });
            view! {
                <List spacing=0.0>
                    <Switch
                        @node_ref=&switch
                        on=false
                        on_change={move |on: bool| set_compact.set(on)}
                    />
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList
                            @node_ref=&list
                            count=VIRTUAL_ITEM_COUNT
                            item_size={item_height}
                        >
                            {move |index: usize| {
                                sink.borrow_mut().push(index);
                                let padding = padding.clone();
                                view! {
                                    <Frame padding_horizontal=0.0 padding_vertical={padding}>
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
    let (switch, scroll, list) = (switch.get(), scroll.get(), list.get());

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let index = 120;
    harness
        .document
        .set_scroll_offset(scroll, index as f32 * VIRTUAL_ITEM_HEIGHT + inset);
    harness.frame(Vec::new());
    let rows = harness.document.children(list);
    let top = harness.rect(rows[0]).top();
    built.borrow_mut().clear();

    harness.click(harness.center(switch));
    harness.frame(Vec::new());

    let skipped = (inset / (VIRTUAL_ITEM_HEIGHT / 2.0)) as usize;
    let remainder = inset - skipped as f32 * VIRTUAL_ITEM_HEIGHT / 2.0;
    let settled = top + skipped as f32 * VIRTUAL_ITEM_HEIGHT / 2.0;
    let kept = rows[skipped];
    assert!(styled::switch_on(harness.document(), switch));
    assert_eq!(harness.document.children(list)[0], kept);
    assert!(
        built
            .borrow()
            .iter()
            .all(|&built| built >= index + rows.len())
    );
    assert_eq!(
        harness.rect(harness.document.children(list)[0]).top(),
        settled
    );
    assert_eq!(
        harness.document.scroll_offset(scroll),
        index as f32 * VIRTUAL_ITEM_HEIGHT / 2.0 + inset
    );

    built.borrow_mut().clear();
    harness.click(harness.center(switch));
    harness.frame(Vec::new());

    assert_eq!(harness.document.children(list)[0], kept);
    assert!(built.borrow().is_empty());
    assert_eq!(
        harness.rect(harness.document.children(list)[0]).top(),
        settled
    );
    assert_eq!(
        harness.document.scroll_offset(scroll),
        (index + skipped) as f32 * VIRTUAL_ITEM_HEIGHT + remainder
    );
}
