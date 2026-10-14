use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::reactive::{BackHandler, NodeRef, Text, build, view};

#[test]
fn a_back_handler_that_takes_the_gesture_hears_each_phase_and_stays_put() {
    let _root = crate::reactive::RootScope::new();
    let (heard, set_heard) = create_signal(Vec::<BackGesture>::new());
    let page = NodeRef::new();
    let document = build(move || {
        view! {
            <BackHandler
                on_gesture={move |gesture: BackGesture| set_heard.update(|heard| heard.push(gesture))}
            >
                <Text string="Page" color=Color32::WHITE @node_ref={&page} />
            </BackHandler>
        }
    });

    let mut harness = Harness::new(document);
    assert!(harness.frame(Vec::new()).window.handles_back);
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
        heard.get_untracked(),
        [started, BackGesture::Progressed(1.0), BackGesture::Invoked]
    );
}
