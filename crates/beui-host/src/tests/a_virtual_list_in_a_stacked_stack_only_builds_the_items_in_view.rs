use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{Container, narrower_than};

const BREAKPOINT: f32 = 500.0;
const STACKED_LIST_HEIGHT: f32 = 100.0;

#[test]
fn a_virtual_list_in_a_stacked_stack_only_builds_the_items_in_view() {
    let _root = crate::reactive::RootScope::new();
    let built = create_signal(Vec::new());
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Container>|_| {
                let narrow = narrower_than(BREAKPOINT);
                let length = create_memo(move || narrow.get().then_some(STACKED_LIST_HEIGHT));
                let grow = create_memo(move || if narrow.get() { 0.0 } else { 1.0 });
                view! {
                    <Stack spacing=Space::None breakpoint=BREAKPOINT>
                        <Spacer grow=0.5 />
                        <ListItem grow=0.5>
                            <List spacing=0.0>
                                <ListItem length grow>
                                    <List spacing=0.0>
                                        <ListItem grow=1.0>
                                            <VirtualList
                                                @node_ref=&scroll
                                                keys={indices(VIRTUAL_ITEM_COUNT)}
                                                item_size=VIRTUAL_ITEM_HEIGHT
                                            >|index: ReadSignal<usize>| {
                                                create_binding(move || {
                                                    let index = index.get();
                                                    built.update(|built| built.push(index));
                                                });
                                                view! {
                                                    <Frame
                                                        padding={Sides::symmetric(0.0, VIRTUAL_ITEM_HEIGHT / 2.0)}
                                                    >
                                                        <Frame />
                                                    </Frame>
                                                }
                                            }</VirtualList>
                                        </ListItem>
                                    </List>
                                </ListItem>
                            </List>
                        </ListItem>
                    </Stack>
                }
            }</Container>
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let wide = (WIDE_VIEWPORT.y / VIRTUAL_ITEM_HEIGHT) as usize;
    assert_eq!(harness.document().children(scroll.get()).len(), wide);

    built.update(|built| built.clear());
    harness.resize(VIEWPORT);
    harness.frame(Vec::new());

    let stacked = (STACKED_LIST_HEIGHT / VIRTUAL_ITEM_HEIGHT) as usize;
    assert_eq!(harness.rect(scroll.get()).height(), STACKED_LIST_HEIGHT);
    assert_eq!(harness.document().children(scroll.get()).len(), stacked);
    assert_eq!(built.get_untracked(), Vec::<usize>::new());
}
