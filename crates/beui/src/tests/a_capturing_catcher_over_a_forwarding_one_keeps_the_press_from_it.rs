use super::*;
use crate::ForwardedInput;
use crate::input::PointerPress;
use crate::primitives::*;
use crate::reactive::*;

fn presses(forwarded: ReadSignal<Vec<ForwardedInput>>) -> usize {
    forwarded.with_untracked(|forwarded| {
        forwarded
            .iter()
            .flat_map(|input| flattened(input.clone()))
            .filter(|event| matches!(event, Event::PointerButton { pressed: true, .. }))
            .count()
    })
}

#[test]
fn a_capturing_catcher_over_a_forwarding_one_keeps_the_press_from_it() {
    let _root = crate::reactive::RootScope::new();
    let forwarded = create_signal(Vec::<ForwardedInput>::new());
    let pressed = create_signal(0);
    let document = build_node(move || {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                    <Interactive
                        on_forward={move |input| forwarded.update(|forwarded| forwarded.push(input))}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=50.0 y=0.0 width=10.0 height=200.0>
                    <Interactive
                        capture_presses=true
                        on_press={move |_: PointerPress| pressed.update(|pressed| *pressed += 1)}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(pos2(55.0, 100.0));
    assert_eq!(pressed.get_untracked(), 1);
    assert_eq!(
        presses(forwarded.read_only()),
        0,
        "the catcher beneath is covered"
    );

    harness.click(pos2(150.0, 100.0));
    assert_eq!(presses(forwarded.read_only()), 1);
}
