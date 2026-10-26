use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::*;
use crate::dock::unstyled::{DockPane, DockTab, DockingLayout};

type Heard = Rc<RefCell<Vec<(u64, bool)>>>;

#[component]
fn Control(id: u64, heard: Heard) -> NodeId {
    view! {
        <Interactive
            focusable=true
            @test_id={format!("control.{id}")}
            on_focus_change={move |focused: bool| heard.borrow_mut().push((id, focused))}
        >
            <Frame width=80.0 height=30.0 />
        </Interactive>
    }
}

#[test]
fn a_tab_hidden_while_it_has_the_focus_hands_the_focus_to_the_tab_shown() {
    let _root = crate::reactive::RootScope::new();
    let heard: Heard = Rc::default();
    let layout = DockingLayout::new();
    let (one, two) = (Rc::clone(&heard), Rc::clone(&heard));
    let document = build_node(move || {
        view! {
            <dock::styled::Docking layout>
                <DockPane id="tabs">
                    <DockTab id=1u64 title="One">
                        <Control id=1 heard=Rc::clone(&one) />
                    </DockTab>
                    <DockTab id=2u64 title="Two">
                        <Control id=2 heard=Rc::clone(&two) />
                    </DockTab>
                </DockPane>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    layout.show(&1);
    harness.frame(Vec::new());
    let first = harness.find("control.1");
    harness.click(harness.center(first));
    assert_eq!(harness.document().focused_node(), Some(first));
    heard.borrow_mut().clear();

    layout.show(&2);
    harness.frame(Vec::new());
    let second = harness.find("control.2");
    assert_eq!(
        *heard.borrow(),
        [(1, false), (2, true)],
        "the hidden control is told it lost the focus, and the shown one gets it"
    );
    assert_eq!(harness.document().focused_node(), Some(second));

    heard.borrow_mut().clear();
    layout.show(&1);
    harness.frame(Vec::new());
    assert_eq!(*heard.borrow(), [(2, false), (1, true)]);
    assert_eq!(
        harness.document().focused_node(),
        Some(first),
        "showing the first tab again brings the focus back to where it was in it"
    );
}
