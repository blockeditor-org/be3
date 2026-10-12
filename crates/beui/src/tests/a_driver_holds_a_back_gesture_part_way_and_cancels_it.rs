use std::cell::RefCell;

use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::reactive::{BackHandler, Text, build, view};

#[test]
fn a_driver_holds_a_back_gesture_part_way_and_cancels_it() {
    let heard = Rc::new(RefCell::new(Vec::<BackGesture>::new()));
    let reports = Rc::clone(&heard);
    let document = build(move || {
        view! {
            <BackHandler
                on_gesture={move |gesture: BackGesture| reports.borrow_mut().push(gesture)}
            >
                <Text string="Page" />
            </BackHandler>
        }
    });
    let mut driven = Driven::new(document);

    driven.ask(&["back", "right"]).expect("the gesture starts");
    driven
        .ask(&["back", "0.4"])
        .expect("it is dragged part way");
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(state.contains("back gesture at 0.4"), "{state}");
    driven.ask(&["back", "cancel"]).expect("it is let go");
    driven.ask(&["back"]).expect("a whole gesture goes back");
    assert_eq!(
        *heard.borrow(),
        [
            BackGesture::Started {
                edge: BackEdge::Right
            },
            BackGesture::Progressed(0.4),
            BackGesture::Cancelled,
            BackGesture::Started {
                edge: BackEdge::Left
            },
            BackGesture::Progressed(0.5),
            BackGesture::Progressed(1.0),
            BackGesture::Invoked,
        ]
    );
    assert!(
        driven.ask(&["back", "commit"]).is_err(),
        "no gesture is left to commit"
    );
}
