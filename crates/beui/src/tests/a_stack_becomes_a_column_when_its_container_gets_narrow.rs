use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::Container;

const BREAKPOINT: f32 = 500.0;
const ITEM_HEIGHT: f32 = 20.0;

#[test]
fn a_stack_becomes_a_column_when_its_container_gets_narrow() {
    let _root = crate::reactive::RootScope::new();
    let (left, right) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <Container>|_|
                <Stack spacing=Space::None breakpoint=BREAKPOINT>
                    <Frame @sizing=ItemSize::Percent(50.0) @node_ref=&left height=ITEM_HEIGHT>
                        <Spacer />
                    </Frame>
                    <Frame @sizing=ItemSize::Percent(50.0) @node_ref=&right height=ITEM_HEIGHT>
                        <Spacer />
                    </Frame>
                </Stack>
            </Container>
        }
    });
    let (left, right) = (left.get(), right.get());

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left).width(), WIDE_VIEWPORT.x / 2.0);
    assert_eq!(harness.rect(right).left(), WIDE_VIEWPORT.x / 2.0);
    assert_eq!(harness.rect(right).top(), harness.rect(left).top());

    harness.resize(VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left).width(), VIEWPORT.x);
    assert_eq!(harness.rect(right).width(), VIEWPORT.x);
    assert_eq!(harness.rect(right).top(), harness.rect(left).bottom());
}
