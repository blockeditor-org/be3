use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Frame, Interactive, NodeRef, build, create_signal, view};
use crate::styled::Dialog;

#[test]
fn a_catcher_in_a_dialog_hears_a_press_on_it() {
    let (open, _set_open) = create_signal(true);
    let heard = Rc::new(RefCell::new(Vec::<Event>::new()));
    let catcher = NodeRef::new();
    let document = build({
        let heard = Rc::clone(&heard);
        let catcher = catcher.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Dialog open={open} title="Dialog" width=400.0 on_dismiss={|| {}}>
                        <Interactive
                            @node_ref={&catcher}
                            on_forward={move |input: ForwardedInput| {
                                heard.borrow_mut().extend(input.events)
                            }}
                        >
                            <Frame height=200.0 />
                        </Interactive>
                    </Dialog>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let rect = harness.rect(catcher.get());
    harness.press_at(rect.center());

    assert!(
        heard
            .borrow()
            .iter()
            .any(|event| matches!(event, Event::PointerButton { pressed: true, .. })),
        "{:?}",
        heard.borrow()
    );
}
