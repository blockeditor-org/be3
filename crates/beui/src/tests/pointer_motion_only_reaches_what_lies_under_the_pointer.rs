use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::{Frame, Interactive, List, NodeRef, build, view};

#[test]
fn pointer_motion_only_reaches_what_lies_under_the_pointer() {
    let hovers: Rc<RefCell<Vec<bool>>> = Rc::default();
    let (top, bottom) = (NodeRef::new(), NodeRef::new());
    let mut document = build({
        let hovers = hovers.clone();
        let (top, bottom) = (top.clone(), bottom.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Interactive
                        @node_ref=&top
                        on_hover_change={move |hovered: bool| hovers.borrow_mut().push(hovered)}
                    >
                        <Frame height=100.0 />
                    </Interactive>
                    <Interactive @node_ref=&bottom on_click={|| {}}>
                        <Frame height=100.0 />
                    </Interactive>
                </List>
            }
        }
    });
    let (top, bottom) = (top.get(), bottom.get());
    let top_counts = counted_with_measures(&mut document, top);
    let bottom_counts = counted_with_measures(&mut document, bottom);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    for x in [10.0, 20.0, 30.0] {
        harness.frame(vec![Event::PointerMoved(pos2(x, 50.0))]);
    }
    assert_eq!(
        top_counts.interactions.get(),
        3,
        "the catcher under the pointer hears every move"
    );
    assert_eq!(
        bottom_counts.interactions.get(),
        0,
        "a catcher away from the pointer is not visited"
    );
    assert_eq!(*hovers.borrow(), vec![true]);

    harness.frame(vec![Event::PointerMoved(pos2(10.0, 500.0))]);
    assert_eq!(
        *hovers.borrow(),
        vec![true, false],
        "the catcher the pointer left hears it leave"
    );
    harness.frame(vec![Event::PointerMoved(pos2(20.0, 500.0))]);
    assert_eq!(
        top_counts.interactions.get(),
        4,
        "once it let go of its hover it is no longer visited"
    );
}
