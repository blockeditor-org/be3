use super::*;
use crate::reactive::{Frame, ReadSignal, Spacer, build, view};
use crate::unstyled::Container;

const PANEL_WIDTH: f32 = 200.0;
const ITEM_HEIGHT: f32 = 20.0;

#[test]
fn a_closure_child_receives_the_handle_its_slot_hands_over() {
    let measured: Rc<RefCell<Option<ReadSignal<Vec2>>>> = Rc::default();
    let item = NodeRef::new();

    let document = build({
        let (measured, item) = (measured.clone(), item.clone());
        move || {
            view! {
                <Frame width=PANEL_WIDTH>
                    <Container>
                        {move |size: ReadSignal<Vec2>| {
                            *measured.borrow_mut() = Some(size);
                            view! { <Frame @node_ref=&item height=ITEM_HEIGHT><Spacer /></Frame> }
                        }}
                    </Container>
                </Frame>
            }
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let measured = measured.borrow();
    let size = measured
        .as_ref()
        .expect("the closure child must have been called with the container size");
    assert_eq!(size.get().x, PANEL_WIDTH);
    assert_eq!(harness.rect(item.get()).width(), PANEL_WIDTH);
}
