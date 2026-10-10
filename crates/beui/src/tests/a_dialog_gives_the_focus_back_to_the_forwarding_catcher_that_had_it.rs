use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{
    Canvas, CanvasItem, Frame, Interactive, NodeRef, build, create_signal, view,
    with_reactive_scope,
};
use crate::styled::{Dialog, TextInput};

#[test]
fn a_dialog_gives_the_focus_back_to_the_forwarding_catcher_that_had_it() {
    let (open, set_open) = create_signal(false);
    let catcher = NodeRef::new();
    let field = NodeRef::new();
    let heard = Rc::new(RefCell::new(Vec::<(bool, Vec<Event>)>::new()));
    let document = build({
        let (open, catcher, field) = (open.clone(), catcher.clone(), field.clone());
        let heard = Rc::clone(&heard);
        let closing = set_open.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Canvas>
                        <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                            <Interactive
                                @node_ref=&catcher
                                focusable=true
                                on_forward={move |input: ForwardedInput| {
                                    heard.borrow_mut().push((input.focused, input.events))
                                }}
                            >
                                <Frame />
                            </Interactive>
                        </CanvasItem>
                    </Canvas>
                    <Dialog open={open.clone()} title="Find" on_dismiss={move || closing.set(false)}>
                        <TextInput @node_ref=&field value="" focused={open} />
                    </Dialog>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.document_mut().focus_focusable(catcher.get());
    assert!(
        harness.document().focus_is_within(catcher.get()));

    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(field.get()));

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(catcher.get()),
        "closing the dialog gives the focus back to the catcher"
    );

    heard.borrow_mut().clear();
    harness.frame(vec![Event::Text("a".to_owned())]);
    assert!(
        heard.borrow().iter().any(|(focused, events)| *focused
            && events
                .iter()
                .any(|event| matches!(event, Event::Text(text) if text == "a"))),
        "and the keys reach it again, {:?}",
        heard.borrow()
    );
}
