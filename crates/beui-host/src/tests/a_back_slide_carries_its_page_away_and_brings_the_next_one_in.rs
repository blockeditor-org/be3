use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::BackSlide;
use beui::input::{BackEdge, BackGesture};

#[test]
fn a_back_slide_carries_its_page_away_and_brings_the_next_one_in() {
    let _root = crate::reactive::RootScope::new();
    let depth = create_signal(2_u32);
    let page = NodeRef::new();
    let document = build_node(move || {
        let nested = create_memo(move || depth.get() > 0);
        view! {
            <BackSlide enabled={nested} on_back={move || depth.set(depth.get_untracked() - 1)}>
                <Text color=Color32::WHITE @node_ref={&page}>"Page"</Text>
            </BackSlide>
        }
    });

    let mut harness = Harness::new(document);
    assert!(
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .handles_back
    );
    let resting = harness.rect(page.get());

    harness.frame(vec![
        Input::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Input::Back(BackGesture::Progressed(1.0)),
    ]);
    let pulled = harness.rect(page.get());
    assert!(
        pulled.left() > resting.left() + 100.0,
        "held all the way, the page has moved well over: {pulled:?} from {resting:?}"
    );

    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    harness.frame(Vec::new());
    let leaving = harness.rect(page.get());
    assert!(leaving.left() > pulled.left());
    assert_eq!(depth.get_untracked(), 2, "it goes back once it is gone");

    let mut gone = false;
    let mut entered = false;
    for _ in 0..60 {
        harness.frame(Vec::new());
        let left = harness.rect(page.get()).left();
        if depth.get_untracked() == 1 {
            gone = true;
        }
        if gone && left < resting.left() {
            entered = true;
        }
        assert!(
            gone || left >= leaving.left(),
            "it never comes back before it has gone"
        );
    }
    assert!(
        gone && entered,
        "the next page slides in from the other side"
    );
    harness.settle();
    assert_eq!(harness.rect(page.get()), resting);

    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    harness.frame(Vec::new());
    assert_eq!(
        depth.get_untracked(),
        1,
        "back with no gesture before it plays the same slide-away"
    );
    assert!(harness.rect(page.get()).left() > resting.left());
    harness.settle();
    assert_eq!(depth.get_untracked(), 0, "and goes back once it is gone");
    assert!(
        !harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .handles_back
    );
}
