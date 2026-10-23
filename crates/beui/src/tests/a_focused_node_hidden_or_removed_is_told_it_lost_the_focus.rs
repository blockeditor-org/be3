use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::primitives::*;
use crate::reactive::*;

type Heard = Rc<RefCell<Vec<(&'static str, bool)>>>;

#[component]
fn Control(name: &'static str, heard: Heard) -> NodeId {
    view! {
        <Interactive
            focusable=true
            @test_id={name}
            on_focus_change={move |focused: bool| heard.borrow_mut().push((name, focused))}
        >
            <Frame width=80.0 height=30.0 />
        </Interactive>
    }
}

#[test]
fn a_focused_node_hidden_or_removed_is_told_it_lost_the_focus() {
    let _root = crate::reactive::RootScope::new();
    let heard: Heard = Rc::default();
    let shown = create_signal(None);
    let kept = create_signal(true);
    let mut controls = None;
    let listening = Rc::clone(&heard);
    let document = build_node(|| {
        let first = view! {
            <Control name="first" heard=Rc::clone(&listening) />
        };
        let second = view! {
            <Control name="second" heard=Rc::clone(&listening) />
        };
        controls = Some((first, second));
        let removable = Rc::clone(&listening);
        view! {
            <List spacing=0.0>
                <Portal node={shown} />
                <Show condition={kept}>
                    <Control name="removable" heard=Rc::clone(&removable) />
                </Show>
            </List>
        }
    });
    let (first, second) = controls.expect("the controls were built");
    let mut harness = Harness::new(document);
    shown.set(Some(second));
    harness.frame(Vec::new());
    shown.set(Some(first));
    harness.frame(Vec::new());
    harness.click(harness.center(first));
    assert_eq!(harness.document().focused_node(), Some(first));
    heard.borrow_mut().clear();

    shown.set(Some(second));
    harness.frame(Vec::new());
    assert_eq!(*heard.borrow(), [("first", false)]);
    assert_eq!(
        harness.document().focused_node(),
        None,
        "a node the portal stopped showing loses the focus"
    );

    let removable = harness.find("removable");
    harness.click(harness.center(removable));
    assert_eq!(harness.document().focused_node(), Some(removable));
    heard.borrow_mut().clear();
    kept.set(false);
    harness.frame(Vec::new());
    assert_eq!(
        *heard.borrow(),
        [("removable", false)],
        "a focused node that is removed is told it lost the focus"
    );
    assert_eq!(harness.document().focused_node(), None);
}
