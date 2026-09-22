use super::*;
use crate::reactive::{ItemSize, List, VirtualList, build, view};
use crate::unstyled::Scroll;

#[test]
fn keyboard_scrolling_reaches_virtual_items_and_endpoints() {
    let scroll = NodeRef::new();
    let document = build({
        let scroll = scroll.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList
                            keys={indices(VIRTUAL_ITEM_COUNT)}
                            item_size=VIRTUAL_ITEM_HEIGHT
                        >
                            {move |_: usize| {
                                view! {
                                    <Frame
                                        padding_horizontal=0.0
                                        padding_vertical={VIRTUAL_ITEM_HEIGHT / 2.0}
                                    >
                                        <Spacer />
                                    </Frame>
                                }
                            }}
                        </VirtualList>
                    </Scroll>
                </List>
            }
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::PageDown, Modifiers::NONE);
    assert_eq!(harness.document.scroll_offset(scroll), VIEWPORT.y);
    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(
        harness.document.scroll_offset(scroll),
        VIRTUAL_ITEM_COUNT as f32 * VIRTUAL_ITEM_HEIGHT - VIEWPORT.y
    );
    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::SHIFT);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
