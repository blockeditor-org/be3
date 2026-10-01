use std::cell::RefCell;

use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::reactive::{BackHandler, NodeRef, Text, build, view};

#[test]
fn a_back_handler_that_takes_the_gesture_hears_each_phase_and_stays_put() {
    let heard = Rc::new(RefCell::new(Vec::<BackGesture>::new()));
    let reports = Rc::clone(&heard);
    let page = NodeRef::new();
    let page_ref = page.clone();
    let document = build(move || {
        view! {
            <BackHandler
                on_gesture={move |gesture: BackGesture| reports.borrow_mut().push(gesture)}
            >
                <Text string="Page" font_size=14.0 color=Color32::WHITE @node_ref={&page_ref} />
            </BackHandler>
        }
    });

    let mut harness = Harness::new(document);
    assert!(harness.frame(Vec::new()).handles_back);
    let resting = harness.rect(page.get());

    let started = BackGesture::Started {
        edge: BackEdge::Left,
    };
    harness.frame(vec![
        Event::Back(started),
        Event::Back(BackGesture::Progressed(1.0)),
    ]);
    assert_eq!(
        harness.rect(page.get()),
        resting,
        "the content is left for the taker to move"
    );

    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    assert_eq!(
        *heard.borrow(),
        [started, BackGesture::Progressed(1.0), BackGesture::Invoked]
    );
}
