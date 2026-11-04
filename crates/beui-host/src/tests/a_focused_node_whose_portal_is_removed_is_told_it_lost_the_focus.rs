use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_focused_node_whose_portal_is_removed_is_told_it_lost_the_focus() {
    let _root = crate::reactive::RootScope::new();
    let heard: Rc<RefCell<Vec<bool>>> = Rc::default();
    let held = create_signal(true);
    let mut moved = None;
    let listening = Rc::clone(&heard);
    let document = build_node(|| {
        let control = view! {
            <Interactive
                focusable=true
                on_focus_change={move |focused: bool| listening.borrow_mut().push(focused)}
            >
                <Frame width=80.0 height=30.0 />
            </Interactive>
        };
        moved = Some(control);
        view! {
            <List spacing=0.0>
                <Show condition={held}>
                    <List spacing=0.0>
                        <Portal node={Some(control)} />
                    </List>
                </Show>
            </List>
        }
    });
    let control = moved.expect("the control was built");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(control));
    assert_eq!(harness.document().focused_node(), Some(control));
    heard.borrow_mut().clear();

    held.set(false);
    harness.frame(Vec::new());
    assert!(
        harness.document().contains(control),
        "the node outlives the portal that showed it"
    );
    assert_eq!(harness.document().focused_node(), None);
    assert_eq!(
        *heard.borrow(),
        [false],
        "a focused node is told it lost the focus when what showed it goes, so it draws no stale focus ring"
    );
}
