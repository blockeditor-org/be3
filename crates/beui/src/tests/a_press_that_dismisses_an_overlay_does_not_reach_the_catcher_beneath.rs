use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, Text, build, create_signal, view};
use crate::styled::Dialog;

const WIDE: Vec2 = Vec2::new(1600.0, 1000.0);

fn presses(heard: &RefCell<Vec<Event>>) -> usize {
    heard
        .borrow()
        .iter()
        .filter(|event| matches!(event, Event::PointerButton { pressed: true, .. }))
        .count()
}

#[test]
fn a_press_that_dismisses_an_overlay_does_not_reach_the_catcher_beneath() {
    let (open, set_open) = create_signal(true);
    let reopen = set_open.clone();
    let shown = open.clone();
    let heard = Rc::new(RefCell::new(Vec::<Event>::new()));
    let document = build({
        let heard = Rc::clone(&heard);
        move || {
            view! {
                <List spacing=0.0>
                    <Canvas @sizing=ItemSize::Percent(100.0)>
                        <CanvasItem x=0.0 y=0.0 width={WIDE.x} height={WIDE.y}>
                            <Interactive
                                on_forward={move |input: ForwardedInput| {
                                    heard.borrow_mut().extend(input.events)
                                }}
                            >
                                <Frame />
                            </Interactive>
                        </CanvasItem>
                    </Canvas>
                    <Dialog
                        open={open}
                        title="Event"
                        width=300.0
                        on_dismiss={move || set_open.set(false)}
                    >
                        <Text string="Body" font_size=14.0 color=Color32::WHITE />
                    </Dialog>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, WIDE);
    harness.frame(Vec::new());

    let outside = pos2(20.0, 20.0);
    harness.frame(vec![Event::PointerMoved(outside)]);
    harness.frame(
        [true, false]
            .into_iter()
            .map(|pressed| Event::PointerButton {
                pos: outside,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            })
            .collect(),
    );
    harness.settle();
    assert!(
        !shown.get_untracked(),
        "the press outside closes the dialog"
    );
    assert_eq!(
        presses(&heard),
        0,
        "the press that closed the dialog is not also the catcher's, {:?}",
        heard.borrow()
    );

    harness.click(outside);
    assert_eq!(
        presses(&heard),
        1,
        "the next press reaches the catcher, {:?}",
        heard.borrow()
    );

    with_installed(harness.document_mut(), |_| reopen.set(true));
    harness.settle();
    harness.click(outside);
    harness.settle();
    assert!(!shown.get_untracked());
    assert_eq!(
        presses(&heard),
        1,
        "a press and release in frames of their own are kept from the catcher too, {:?}",
        heard.borrow()
    );
}
