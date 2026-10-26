use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn flipping_a_switch_can_replace_the_items_of_a_scroll() {
    let _root = crate::reactive::RootScope::new();
    for inset in [7.0, 17.0] {
        check_compact_rows(inset);
    }
}

fn check_compact_rows(inset: f32) {
    let built = create_signal(Vec::new());
    let (switch, scroll, list) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        let compact = create_signal(false);
        let item_height = create_memo(move || {
            if compact.get() {
                VIRTUAL_ITEM_HEIGHT / 2.0
            } else {
                VIRTUAL_ITEM_HEIGHT
            }
        });
        let padding = create_memo(move || item_height.get() / 2.0);
        view! {
            <List spacing=0.0>
                <Switch @node_ref=&switch on={compact} />
                <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <VirtualList
                        @node_ref=&list
                        keys={indices(VIRTUAL_ITEM_COUNT)}
                        item_size={item_height}
                    >|index: ReadSignal<usize>| {
                        create_binding(move || {
                            let index = index.get();
                            built.update(|built| built.push(index));
                        });
                        view! {
                            <Frame
                                padding={create_memo(move || Sides::symmetric(0.0, padding.get()))}
                            >
                                <Spacer />
                            </Frame>
                        }
                    }</VirtualList>
                </Offset>
            </List>
        }
    });
    let (switch, scroll, list) = (switch.get(), scroll.get(), list.get());

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let index = 120;
    harness
        .document_mut()
        .set_scroll_offset(scroll, index as f32 * VIRTUAL_ITEM_HEIGHT + inset);
    harness.frame(Vec::new());
    let rows = harness.document().children(list);
    let top = harness.rect(rows[0]).top();
    built.update(|built| built.clear());

    harness.click(harness.center(switch));
    harness.frame(Vec::new());

    let skipped = (inset / (VIRTUAL_ITEM_HEIGHT / 2.0)) as usize;
    let remainder = inset - skipped as f32 * VIRTUAL_ITEM_HEIGHT / 2.0;
    let settled = top + skipped as f32 * VIRTUAL_ITEM_HEIGHT / 2.0;
    let kept = rows[skipped];
    assert!(
        harness
            .document()
            .api::<styled::ToggleApi>(switch)
            .checked()
            .get()
    );
    assert_eq!(harness.document().children(list)[0], kept);
    assert!(
        built
            .get_untracked()
            .iter()
            .all(|&built| built >= index + rows.len())
    );
    assert_eq!(
        harness.rect(harness.document().children(list)[0]).top(),
        settled
    );
    assert_eq!(
        harness.document().scroll_offset(scroll),
        index as f32 * VIRTUAL_ITEM_HEIGHT / 2.0 + inset
    );

    built.update(|built| built.clear());
    harness.click(harness.center(switch));
    harness.frame(Vec::new());

    assert_eq!(harness.document().children(list)[0], kept);
    assert!(built.with_untracked(|built| built.is_empty()));
    assert_eq!(
        harness.rect(harness.document().children(list)[0]).top(),
        settled
    );
    assert_eq!(
        harness.document().scroll_offset(scroll),
        (index + skipped) as f32 * VIRTUAL_ITEM_HEIGHT + remainder
    );
}
