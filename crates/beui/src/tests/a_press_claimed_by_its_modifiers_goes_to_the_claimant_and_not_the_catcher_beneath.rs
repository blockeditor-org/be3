use super::*;
use crate::ForwardedInput;
use crate::primitives::*;
use crate::reactive::*;
use crate::{Claim, HitRegion};

fn presses(log: ReadSignal<Vec<Event>>) -> usize {
    log.with_untracked(|log| {
        log.iter()
            .filter(|event| matches!(event, Event::PointerButton { .. }))
            .count()
    })
}

#[test]
fn a_press_claimed_by_its_modifiers_goes_to_the_claimant_and_not_the_catcher_beneath() {
    let _root = crate::reactive::RootScope::new();
    let child = create_signal(Vec::<Event>::new());
    let parent = create_signal(Vec::<Event>::new());
    let document = build_node(move || {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=200.0>
                    <Interactive
                        on_forward={move |input: ForwardedInput| {
                            child.update(|child| child.extend(flattened(input)))
                        }}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                    <Interactive
                        hit_area={HitRegion::rect(Rect::from_min_max(pos2(100.0, 0.0), pos2(200.0, 200.0)))}
                        claims={vec![Claim::holding(Modifiers::LOGO, true)]}
                        on_forward={move |input: ForwardedInput| {
                            parent.update(|parent| parent.extend(flattened(input)))
                        }}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let hole = pos2(50.0, 50.0);

    harness.click(hole);
    assert_eq!(
        presses(child.read_only()),
        2,
        "a plain click in the hole is the child's"
    );
    assert_eq!(presses(parent.read_only()), 0);

    drag_with(&mut harness, hole, pos2(70.0, 90.0), Modifiers::LOGO);
    assert_eq!(
        presses(child.read_only()),
        2,
        "the child hears nothing of a press the parent claimed"
    );
    assert_eq!(
        presses(parent.read_only()),
        2,
        "the parent hears the press it claimed and its release"
    );
    assert!(
        parent
            .get_untracked()
            .contains(&Event::PointerMoved(pos2(70.0, 90.0))),
        "and the drag between them"
    );
}
