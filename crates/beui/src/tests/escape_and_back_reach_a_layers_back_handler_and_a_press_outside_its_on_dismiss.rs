use super::*;
use crate::Dismiss;
use crate::base::overlay::{OverlayAnchor, Placement};
use crate::input::BackGesture;
use crate::reactive::{BackHandler, Frame, Overlay, build, view};

#[test]
fn escape_and_back_reach_a_layers_back_handler_and_a_press_outside_its_on_dismiss() {
    let backs = Rc::new(Cell::new(0));
    let dismissed = Rc::new(RefCell::new(Vec::new()));
    let document = build({
        let (backs, dismissed) = (backs.clone(), dismissed.clone());
        move || {
            view! {
                <Frame>
                    <Overlay
                        anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                        placement=Placement::At
                        open=true
                        on_dismiss={move |reason: Dismiss| dismissed.borrow_mut().push(reason)}
                    >
                        <BackHandler on_back={move || backs.set(backs.get() + 1)}>
                            <Frame width=40.0 height=30.0 />
                        </BackHandler>
                    </Overlay>
                </Frame>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(backs.get(), 1, "Escape goes to the layer's back handler");
    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    assert_eq!(backs.get(), 2, "and so does back");
    assert!(dismissed.borrow().is_empty());

    harness.click(pos2(VIEWPORT.x - 5.0, VIEWPORT.y - 5.0));
    harness.frame(Vec::new());
    assert_eq!(
        *dismissed.borrow(),
        [Dismiss::OutsidePress],
        "a press outside asks the layer itself"
    );
    assert_eq!(backs.get(), 2);
}
