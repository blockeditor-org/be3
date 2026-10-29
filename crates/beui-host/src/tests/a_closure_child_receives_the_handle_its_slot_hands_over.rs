use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Container;

const PANEL_WIDTH: f32 = 200.0;
const ITEM_HEIGHT: f32 = 20.0;

#[test]
fn a_closure_child_receives_the_handle_its_slot_hands_over() {
    let _root = crate::reactive::RootScope::new();
    let measured = create_signal::<Option<ReadSignal<Vec2>>>(None);
    let item = NodeRef::new();

    let document = build_node(move || {
        view! {
            <Frame width=PANEL_WIDTH>
                <Container>|size: ReadSignal<Vec2>| {
                    measured.update(|measured| *measured = Some(size));
                    view! {
                        <Frame @node_ref=&item height=ITEM_HEIGHT>
                            <Spacer />
                        </Frame>
                    }
                }</Container>
            </Frame>
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let size = measured
        .get_untracked()
        .expect("the closure child must have been called with the container size");
    assert_eq!(size.get().x, PANEL_WIDTH);
    assert_eq!(harness.rect(item.get()).width(), PANEL_WIDTH);
}
