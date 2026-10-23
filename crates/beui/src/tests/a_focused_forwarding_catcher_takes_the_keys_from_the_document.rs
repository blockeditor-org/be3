use super::*;
use crate::ForwardedInput;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_focused_forwarding_catcher_takes_the_keys_from_the_document() {
    let _root = crate::reactive::RootScope::new();
    let keys = create_signal(Vec::<Event>::new());
    let document = build_node(move || {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                    <Interactive
                        focusable=true
                        on_forward={move |input: ForwardedInput| {
                            if input.focused {
                                keys.update(|keys| keys.extend(flattened(input)));
                            }
                        }}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                    <Interactive focusable=true>
                        <Frame />
                    </Interactive>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));
    let focused = harness.document().focused_node();
    assert!(focused.is_some());
    keys.update(|keys| keys.clear());

    harness.frame(vec![key_event(Key::Tab, true), Input::Text("a".to_owned())]);
    let tab = Event::key(beui::KeyPress {
        key: Key::Tab,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });

    assert_eq!(keys.get_untracked(), [tab, Event::Text("a".to_owned())]);
    assert_eq!(
        harness.document().focused_node(),
        focused,
        "the document does not also move the focus on the Tab it forwarded"
    );
}
