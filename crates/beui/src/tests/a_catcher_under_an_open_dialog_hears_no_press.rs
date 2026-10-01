use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, Text, build, create_signal, view};
use crate::styled::Dialog;

#[test]
fn a_catcher_under_an_open_dialog_hears_no_press() {
    let (open, _set_open) = create_signal(true);
    let heard = Rc::new(RefCell::new(Vec::<Event>::new()));
    let document = build({
        let heard = Rc::clone(&heard);
        move || {
            view! {
                <List spacing=0.0>
                    <Canvas>
                        <CanvasItem x=0.0 y=0.0 width={VIEWPORT.x} height={VIEWPORT.y}>
                            <Interactive
                                on_forward={move |input: ForwardedInput| {
                                    heard.borrow_mut().extend(input.events)
                                }}
                            >
                                <Frame />
                            </Interactive>
                        </CanvasItem>
                    </Canvas>
                    <Dialog open={open} title="Event" width=300.0 on_dismiss={|| {}}>
                        <Text string="Body" font_size=14.0 color=Color32::WHITE />
                    </Dialog>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.press_at(pos2(10.0, 10.0));

    assert!(
        !heard
            .borrow()
            .iter()
            .any(|event| matches!(event, Event::PointerButton { .. })),
        "the dialog keeps the press from the catcher beneath it, {:?}",
        heard.borrow()
    );
}
