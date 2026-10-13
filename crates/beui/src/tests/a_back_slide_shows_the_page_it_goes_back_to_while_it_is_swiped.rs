use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::reactive::{NodeRef, Text, build, clone, create_memo, create_signal, view};
use crate::unstyled::BackSlide;

#[test]
fn a_back_slide_shows_the_page_it_goes_back_to_while_it_is_swiped() {
    let (depth, set_depth) = create_signal(2_u32);
    let page = NodeRef::new();
    let previous = NodeRef::new();
    let page_ref = page.clone();
    let previous_ref = previous.clone();
    let watched = previous.clone();
    let document = build({
        let depth = depth.clone();
        move || {
            let nested = create_memo(clone!(depth -> move || depth.get() > 0));
            view! {
                <BackSlide
                    enabled={nested}
                    on_back={move || set_depth.set(depth.get_untracked() - 1)}
                    behind={move || view! {
                        <Text
                            string="Previous"
                            font_size=14.0
                            color=Color32::WHITE
                            @node_ref={&previous_ref}
                        />
                    }}
                >
                    <Text string="Page" font_size=14.0 color=Color32::WHITE @node_ref={&page_ref} />
                </BackSlide>
            }
        }
    });

    let behind = move |harness: &Harness| {
        watched
            .try_get()
            .is_some_and(|id| harness.document().contains(id))
    };
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let resting = harness.rect(page.get());
    assert!(
        !behind(&harness),
        "the page behind is only built while back is swiped"
    );

    harness.frame(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Event::Back(BackGesture::Progressed(0.5)),
    ]);
    let started = harness.rect(previous.get());
    assert!(
        started.left() < resting.left(),
        "the page behind waits off to the side it comes in from: {started:?}"
    );

    harness.frame(vec![Event::Back(BackGesture::Progressed(1.0))]);
    let pulled = harness.rect(previous.get());
    assert!(
        pulled.left() > started.left(),
        "the page behind comes in with the finger: {pulled:?} from {started:?}"
    );

    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    let mut last = pulled.left();
    while behind(&harness) {
        let left = harness.rect(previous.get()).left();
        assert!(left >= last, "the page behind never backs away");
        last = left;
        harness.frame(Vec::new());
    }
    assert_eq!(
        depth.get_untracked(),
        1,
        "it goes back once the page is gone"
    );
    assert_eq!(
        harness.rect(page.get()),
        resting,
        "once back has gone, the page is where the one behind was, with nothing left to slide in"
    );
    assert!(!behind(&harness));
    harness.settle();
    assert_eq!(harness.rect(page.get()), resting);

    harness.frame(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Event::Back(BackGesture::Progressed(1.0)),
    ]);
    assert!(behind(&harness));
    harness.frame(vec![Event::Back(BackGesture::Cancelled)]);
    harness.settle();
    assert_eq!(harness.rect(page.get()), resting);
    assert!(
        !behind(&harness),
        "a cancelled swipe puts the page behind away again"
    );
    assert_eq!(depth.get_untracked(), 1);
}
