use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_tap_beside_a_flinging_scroll_reaches_the_control_it_lands_on() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (scroll, beside) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(50.0) @node_ref=&scroll>
                    <ForEach keys={indices(300)}>|index: usize|
                        <LabelledButton label={format!("Row {index}")} on_click={|| {}} />
                    </ForEach>
                </Scroll>
                <Interactive
                    @node_ref=&beside
                    @sizing=ItemSize::Percent(50.0)
                    on_click={move || clicks.update(|clicks| *clicks += 1)}
                >
                    <Frame />
                </Interactive>
            </List>
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
    assert!(
        harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating()
    );

    let tap = harness.center(beside);
    harness.touch(TouchPhase::Start, tap);
    harness.touch(TouchPhase::End, tap);

    assert_eq!(
        clicks.get_untracked(),
        1,
        "a flinging scroll only takes the taps that land on it"
    );
}
