use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

/// A press while the scroll settles on a stop stops it as a press stops a fling: it goes to the
/// stop nearest where it was pressed, without the speed it had.
#[test]
fn a_press_while_a_scroll_settles_on_a_stop_takes_it_to_the_nearest_stop() {
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
        document.offset_value(document.first_offset_within(scroll).expect("an offset"))
    };
    harness.touch(TouchPhase::Start, pos2(100.0, 190.0));
    for y in [160.0, 120.0, 80.0, 40.0] {
        harness.touch(TouchPhase::Move, pos2(100.0, y));
    }
    harness.touch(TouchPhase::End, pos2(100.0, 40.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let pressed_at = offset(&harness);
    assert!(
        harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating(),
        "flung, it is on its way to a stop"
    );
    harness.touch(TouchPhase::Start, pos2(100.0, 100.0));
    harness.touch(TouchPhase::End, pos2(100.0, 100.0));
    harness.settle();
    let nearest = (pressed_at / 200.0).round() * 200.0;
    assert_eq!(
        offset(&harness),
        nearest,
        "pressed at {pressed_at}, it settles on the stop nearest there"
    );
}
