use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

/// A pager: ten pages of 200 points in a 200-point scroll, with a stop at each page.
#[test]
fn a_flung_or_released_scroll_settles_on_its_nearest_snap_stop() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem length=200.0>
                    <Scroll
                        @node_ref=&scroll
                        style={plain_scroll()}
                        snaps={(0..10).map(|page| page as f32 * 200.0).collect::<Vec<f32>>()}
                    >
                        <ForEach value={indices(10)}>|_: usize|
                            <Frame height=200.0 />
                        </ForEach>
                    </Scroll>
                </ListItem>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::sized(document, vec2(200.0, 200.0));
    harness.frame(Vec::new());
    let offset = |harness: &Harness| {
        let document = harness.document();
        document.offset_value(
            document
                .first_offset_within(scroll)
                .expect("a scroll lays its content out in an offset"),
        )
    };

    // Dragged a little over a third of a page and let go slowly: back to the first.
    harness.touch(TouchPhase::Start, pos2(100.0, 150.0));
    for step in 1..=14 {
        harness.touch(TouchPhase::Move, pos2(100.0, 150.0 - step as f32 * 5.0));
        harness.frame(Vec::new());
    }
    harness.touch(TouchPhase::End, pos2(100.0, 80.0));
    harness.settle();
    assert_eq!(offset(&harness), 0.0, "a slow release short of half a page goes back");

    // Dragged past half a page and let go slowly: on to the second.
    harness.touch(TouchPhase::Start, pos2(100.0, 180.0));
    for step in 1..=24 {
        harness.touch(TouchPhase::Move, pos2(100.0, 180.0 - step as f32 * 5.0));
        harness.frame(Vec::new());
    }
    harness.touch(TouchPhase::End, pos2(100.0, 60.0));
    let mut frames = 0;
    while harness
        .document()
        .api::<unstyled::ScrollApi>(scroll)
        .animating()
    {
        assert!(frames < 600, "it never settled");
        let at = offset(&harness);
        assert!(
            (0.0..=400.0).contains(&at),
            "it moves towards the stop, at {at}"
        );
        harness.frame(Vec::new());
        frames += 1;
    }
    assert_eq!(offset(&harness), 200.0, "a release past half a page goes on to the next");

    // Flung hard: on several pages, and still to a stop.
    harness.touch(TouchPhase::Start, pos2(100.0, 190.0));
    for y in [160.0, 120.0, 80.0, 40.0] {
        harness.touch(TouchPhase::Move, pos2(100.0, y));
    }
    harness.touch(TouchPhase::End, pos2(100.0, 40.0));
    harness.settle();
    let flung = offset(&harness);
    assert!(flung > 400.0, "a fling carries it on past the next page, to {flung}");
    assert_eq!(flung % 200.0, 0.0, "and it rests on a page, at {flung}");
}
