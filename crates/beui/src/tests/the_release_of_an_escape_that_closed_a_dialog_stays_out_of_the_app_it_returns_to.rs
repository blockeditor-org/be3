use super::*;
use crate::ForwardedInput;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_release_of_an_escape_that_closed_a_dialog_stays_out_of_the_app_it_returns_to() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let (catcher, field) = (NodeRef::new(), NodeRef::new());
    let heard = create_signal(Vec::<Event>::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                        <Interactive
                            @node_ref=&catcher
                            focusable=true
                            on_forward={move |input: ForwardedInput| {
                                heard.update(|heard| heard.extend(flattened(input)))
                            }}
                        >
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                </Canvas>
                <Dialog open={open} title="Find" on_dismiss={move || open.set(false)}>
                    <TextInput @node_ref=&field value="" focused={open} />
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.document_mut().focus_focusable(catcher.get());
    open.set(true);
    harness.frame(Vec::new());
    assert!(harness.document().focus_is_within(field.get()));

    harness.frame(vec![key_event(Key::Escape, true)]);
    harness.settle();
    assert!(harness.document().focus_is_within(catcher.get()));
    heard.update(|heard| heard.clear());
    harness.frame(vec![key_event(Key::Escape, false)]);
    assert!(
        !heard
            .get_untracked()
            .iter()
            .any(|event| matches!(event, Event::Key { press, .. } if press.key == Key::Escape)),
        "the release goes with the press to the layer that took it, {:?}",
        heard.get_untracked()
    );
}
