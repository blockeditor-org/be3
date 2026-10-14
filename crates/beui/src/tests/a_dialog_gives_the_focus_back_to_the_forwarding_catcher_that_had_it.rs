use super::*;
use crate::ForwardedInput;
use crate::reactive::{
    Canvas, CanvasItem, Frame, Interactive, NodeRef, build, create_signal, view,
    with_reactive_scope,
};
use crate::styled::{Dialog, TextInput};

#[test]
fn a_dialog_gives_the_focus_back_to_the_forwarding_catcher_that_had_it() {
    let _root = crate::reactive::RootScope::new();
    let (open, set_open) = create_signal(false);
    let catcher = NodeRef::new();
    let field = NodeRef::new();
    let (heard, set_heard) = create_signal(Vec::<(bool, Vec<Event>)>::new());
    let document = build({
        move || {
            view! {
                <List spacing=0.0>
                    <Canvas>
                        <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                            <Interactive
                                @node_ref=&catcher
                                focusable=true
                                on_forward={move |input: ForwardedInput| {
                                    set_heard.update(|heard| heard.push((input.focused, flattened(input))))
                                }}
                            >
                                <Frame />
                            </Interactive>
                        </CanvasItem>
                    </Canvas>
                    <Dialog open={open} title="Find" on_dismiss={move || set_open.set(false)}>
                        <TextInput @node_ref=&field value="" focused={open} />
                    </Dialog>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.document_mut().focus_focusable(catcher.get());
    assert!(harness.document().focus_is_within(catcher.get()));

    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());
    assert!(harness.document().focus_is_within(field.get()));

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(catcher.get()),
        "set_open the dialog gives the focus back to the catcher"
    );

    set_heard.update(|heard| heard.clear());
    harness.frame(vec![Event::Text("a".to_owned())]);
    assert!(
        heard
            .get_untracked()
            .iter()
            .any(|(focused, events)| *focused
                && events
                    .iter()
                    .any(|event| matches!(event, Event::Text(text) if text == "a"))),
        "and the keys reach it again, {:?}",
        heard.get_untracked()
    );
}
