use super::*;
use crate::reactive::{Direction, ForEach, ItemSize, List, NodeRef, build, view};
use crate::unstyled::Scroll;

#[test]
fn a_tap_beside_a_flinging_scroll_reaches_the_control_it_lands_on() {
    let clicks = Rc::new(Cell::new(0));
    let (scroll, beside) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (clicks, scroll, beside) = (clicks.clone(), scroll.clone(), beside.clone());
        move || {
            view! {
                <List direction=Direction::Horizontal spacing=0.0>
                    <Scroll @sizing=ItemSize::Percent(50.0) @node_ref=&scroll>
                        <ForEach keys={indices(300)}>
                            {move |index: usize| view! {
                                <LabelledButton label={format!("Row {index}")} on_click={|| {}} />
                            }}
                        </ForEach>
                    </Scroll>
                    <Interactive
                        @node_ref=&beside
                        @sizing=ItemSize::Percent(50.0)
                        on_click={move || clicks.set(clicks.get() + 1)}
                    >
                        <Frame />
                    </Interactive>
                </List>
            }
        }
    });
    let (scroll, beside) = (scroll.get(), beside.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let x = harness.rect(scroll).center().x;
    harness.finger_drag(&[
        pos2(x, 280.0),
        pos2(x, 240.0),
        pos2(x, 200.0),
        pos2(x, 160.0),
    ]);
    harness.frame(Vec::new());
    assert!(unstyled::scroll_animating(harness.document(), scroll));

    let tap = harness.center(beside);
    harness.touch(TouchPhase::Start, tap);
    harness.touch(TouchPhase::End, tap);

    assert_eq!(
        clicks.get(),
        1,
        "a flinging scroll only takes the taps that land on it"
    );
}
