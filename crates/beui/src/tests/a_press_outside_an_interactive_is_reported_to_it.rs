use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::{Frame, Interactive, NodeRef, build, view};

#[test]
fn a_press_outside_an_interactive_is_reported_to_it() {
    let heard = Rc::new(RefCell::new(Vec::<Pos2>::new()));
    let watcher = NodeRef::new();
    let elsewhere = NodeRef::new();
    let document = build({
        let heard = Rc::clone(&heard);
        let watcher = watcher.clone();
        let elsewhere = elsewhere.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Interactive
                        @node_ref={&watcher}
                        on_press_outside={move |pos: Pos2| heard.borrow_mut().push(pos)}
                    >
                        <Frame height=100.0 />
                    </Interactive>
                    <Interactive @node_ref={&elsewhere} on_click={|| {}}>
                        <Frame height=100.0 />
                    </Interactive>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.rect(watcher.get()).center());
    assert!(
        heard.borrow().is_empty(),
        "a press inside is not outside: {:?}",
        heard.borrow()
    );

    let outside = harness.rect(elsewhere.get()).center();
    harness.click(outside);
    assert_eq!(
        *heard.borrow(),
        vec![outside],
        "a press on something else that takes it is still heard, once"
    );

    harness.touch(TouchPhase::Start, outside);
    harness.touch(TouchPhase::End, outside);
    assert_eq!(heard.borrow().len(), 2, "a finger press is a press too");
}
