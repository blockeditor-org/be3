use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_driver_holds_a_back_gesture_part_way_and_cancels_it() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::<BackGesture>::new());
    let document = build_node(move || {
        view! {
            <BackHandler
                on_gesture={move |gesture: BackGesture| heard.update(|heard| heard.push(gesture))}
            >
                <Text>"Page"</Text>
            </BackHandler>
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));

    driven
        .ask(&["app", "back", "right"])
        .expect("the gesture starts");
    driven
        .ask(&["app", "back", "0.4"])
        .expect("it is dragged part way");
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(state.contains("back gesture at 0.4"), "{state}");
    driven
        .ask(&["app", "back", "cancel"])
        .expect("it is let go");
    driven
        .ask(&["app", "back"])
        .expect("a whole gesture goes back");
    assert_eq!(
        heard.get_untracked(),
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
        driven.ask(&["app", "back", "commit"]).is_err(),
        "no gesture is left to commit"
    );
}
