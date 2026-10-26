use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{Container, narrower_than};

const INNER_WIDTH: f32 = 200.0;
const BREAKPOINT: f32 = 300.0;
const ITEM_HEIGHT: f32 = 20.0;

#[test]
fn a_nested_container_reports_its_own_width_not_the_windows() {
    let _root = crate::reactive::RootScope::new();
    let sizes = create_signal::<Vec<ReadSignal<Vec2>>>(Vec::new());
    let collapsed = create_signal::<Vec<Memo<bool>>>(Vec::new());
    let (outer_item, inner_item) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <Container>|size| {
                sizes.update(|sizes| sizes.push(size));
                let narrow = narrower_than(BREAKPOINT);
                collapsed.update(|collapsed| collapsed.push(narrow));
                view! {
                    <List spacing=0.0>
                        <Stack spacing=0.0 breakpoint=BREAKPOINT>
                            <Frame
                                @sizing=ItemSize::Percent(100.0)
                                @node_ref=&outer_item
                                height=ITEM_HEIGHT
                            >
                                <Spacer />
                            </Frame>
                        </Stack>
                        <Frame width=INNER_WIDTH>
                            <Container>|size| {
                                sizes.update(|sizes| sizes.push(size));
                                let narrow = narrower_than(BREAKPOINT);
                                collapsed.update(|collapsed| collapsed.push(narrow));
                                view! {
                                    <Stack spacing=0.0 breakpoint=BREAKPOINT>
                                        <Frame
                                            @sizing=ItemSize::Percent(100.0)
                                            @node_ref=&inner_item
                                            height=ITEM_HEIGHT
                                        >
                                            <Spacer />
                                        </Frame>
                                    </Stack>
                                }
                            }</Container>
                        </Frame>
                    </List>
                }
            }</Container>
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let sizes = sizes.get_untracked();
    assert_eq!(sizes[0].get().x, WIDE_VIEWPORT.x);
    assert_eq!(sizes[1].get().x, INNER_WIDTH);

    let collapsed = collapsed.get_untracked();
    assert!(!collapsed[0].get());
    assert!(collapsed[1].get());

    assert_eq!(harness.rect(outer_item.get()).width(), WIDE_VIEWPORT.x);
    assert_eq!(harness.rect(inner_item.get()).width(), INNER_WIDTH);
}
