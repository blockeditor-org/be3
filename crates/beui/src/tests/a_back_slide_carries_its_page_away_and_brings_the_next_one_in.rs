use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::reactive::{NodeRef, Text, build, clone, create_memo, create_signal, view};
use crate::unstyled::BackSlide;

#[test]
fn a_back_slide_carries_its_page_away_and_brings_the_next_one_in() {
    let (depth, set_depth) = create_signal(2_u32);
    let page = NodeRef::new();
    let page_ref = page.clone();
    let document = build({
        let depth = depth.clone();
        move || {
            let nested = create_memo(clone!(depth -> move || depth.get() > 0));
            view! {
                <BackSlide
                    enabled={nested}
                    on_back={move || set_depth.set(depth.get_untracked() - 1)}
                >
                    <Text string="Page" font_size=14.0 color=Color32::WHITE @node_ref={&page_ref} />
                </BackSlide>
            }
        }
    });

    let mut harness = Harness::new(document);
    assert!(harness.frame(Vec::new()).handles_back);
    let resting = harness.rect(page.get());

    harness.frame(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Event::Back(BackGesture::Progressed(1.0)),
    ]);
    let pulled = harness.rect(page.get());
    assert!(
        pulled.left() > resting.left() + 100.0,
        "held all the way, the page has moved well over: {pulled:?} from {resting:?}"
    );

    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
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

    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    assert_eq!(
        depth.get_untracked(),
        0,
        "back with no gesture before it goes back at once"
    );
    assert!(!harness.frame(Vec::new()).handles_back);
}
