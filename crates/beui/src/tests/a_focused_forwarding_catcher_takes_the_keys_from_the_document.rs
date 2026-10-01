use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ForwardedInput;
use crate::reactive::{Canvas, CanvasItem, Frame, Interactive, build, view};

#[test]
fn a_focused_forwarding_catcher_takes_the_keys_from_the_document() {
    let keys = Rc::new(RefCell::new(Vec::<Event>::new()));
    let document = build({
        let keys = Rc::clone(&keys);
        move || {
            view! {
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                        <Interactive
                            focusable=true
                            on_forward={move |input: ForwardedInput| {
                                if input.focused {
                                    keys.borrow_mut().extend(input.events);
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
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));
    let focused = harness.document().focused_node();
    assert!(focused.is_some());
    keys.borrow_mut().clear();

    let tab = Event::Key {
        key: Key::Tab,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    harness.frame(vec![tab.clone(), Event::Text("a".to_owned())]);

    assert_eq!(*keys.borrow(), [tab, Event::Text("a".to_owned())]);
    assert_eq!(
        harness.document().focused_node(),
        focused,
        "the document does not also move the focus on the Tab it forwarded"
    );
}
