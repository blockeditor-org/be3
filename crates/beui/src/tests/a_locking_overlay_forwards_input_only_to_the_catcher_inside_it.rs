use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::base::overlay::{OverlayAnchor, Placement};
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, Overlay, build, view};

#[test]
fn a_locking_overlay_forwards_input_only_to_the_catcher_inside_it() {
    let outside = Rc::new(RefCell::new(Vec::<Event>::new()));
    let inside = Rc::new(RefCell::new(Vec::<Event>::new()));
    let document = build({
        let (outside, inside) = (Rc::clone(&outside), Rc::clone(&inside));
        move || {
            view! {
                <List spacing=0.0>
                    <Canvas>
                        <CanvasItem x=0.0 y=0.0 width={VIEWPORT.x} height={VIEWPORT.y}>
                            <Interactive
                                focusable=true
                                focused=true
                                on_forward={move |input: ForwardedInput| {
                                    outside.borrow_mut().extend(input.events)
                                }}
                            >
                                <Frame />
                            </Interactive>
                        </CanvasItem>
                    </Canvas>
                    <Overlay
                        anchor=OverlayAnchor::Point(Pos2::ZERO)
                        placement=Placement::Fill
                        scrim=Color32::BLACK
                        locks=true
                        open=true
                    >
                        <Frame width={VIEWPORT.x} height={VIEWPORT.y}>
                            <Interactive
                                focusable=true
                                focused=true
                                on_forward={move |input: ForwardedInput| {
                                    inside.borrow_mut().extend(input.events)
                                }}
                            >
                                <Frame />
                            </Interactive>
                        </Frame>
                    </Overlay>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    outside.borrow_mut().clear();
    inside.borrow_mut().clear();

    harness.click(pos2(10.0, 10.0));
    harness.type_text("hunter2");
    harness.key(Key::Escape, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::ALT);

    assert!(
        outside.borrow().iter().all(|event| matches!(
            event,
            Event::PointerMoved(_) | Event::PointerGone | Event::Focus(false)
        )),
        "nothing under the lock hears a press, a key or text: {:?}",
        outside.borrow()
    );
    let heard = inside.borrow();
    assert!(
        heard
            .iter()
            .any(|event| matches!(event, Event::PointerButton { .. })),
        "the catcher inside the lock hears the press: {heard:?}"
    );
    let typed: String = heard
        .iter()
        .filter_map(|event| match event {
            Event::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(typed, "hunter2", "and the typing");
    assert!(
        heard
            .iter()
            .any(|event| matches!(event, Event::Key { key: Key::Escape, .. })),
        "and Escape, which the lock keeps for itself: {heard:?}"
    );
}
