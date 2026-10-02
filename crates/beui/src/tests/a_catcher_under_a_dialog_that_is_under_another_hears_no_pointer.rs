use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{
    Frame, Interactive, NodeRef, Text, build, create_signal, view, with_reactive_scope,
};
use crate::styled::Dialog;

#[test]
fn a_catcher_under_a_dialog_that_is_under_another_hears_no_pointer() {
    let (open, _set_open) = create_signal(true);
    let (above, set_above) = create_signal(false);
    let heard = Rc::new(RefCell::new(Vec::<Event>::new()));
    let catcher = NodeRef::new();
    let document = build({
        let heard = Rc::clone(&heard);
        let catcher = catcher.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Dialog open={open} title="Below" width=600.0 on_dismiss={|| {}}>
                        <Interactive
                            @node_ref={&catcher}
                            on_forward={move |input: ForwardedInput| {
                                heard.borrow_mut().extend(input.events)
                            }}
                        >
                            <Frame height=300.0 />
                        </Interactive>
                    </Dialog>
                    <Dialog open={above} title="Above" width=200.0 on_dismiss={|| {}}>
                        <Text string="Body" font_size=14.0 color=Color32::WHITE />
                    </Dialog>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    with_reactive_scope(harness.document_mut(), move || set_above.set(true));
    harness.frame(Vec::new());
    let below = harness.rect(catcher.get());
    heard.borrow_mut().clear();
    harness.frame(vec![Event::PointerMoved(below.min + vec2(10.0, 10.0))]);

    assert!(
        heard.borrow().is_empty(),
        "the upper dialog keeps the pointer from the catcher in the one beneath it, {:?}",
        heard.borrow()
    );
}
