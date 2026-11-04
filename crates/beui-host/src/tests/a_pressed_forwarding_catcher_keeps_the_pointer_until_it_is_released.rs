use super::*;
use crate::ForwardedInput;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_pressed_forwarding_catcher_keeps_the_pointer_until_it_is_released() {
    let _root = crate::reactive::RootScope::new();
    let log = create_signal(Vec::<Event>::new());
    let document = build_node(move || {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                    <Interactive
                        on_forward={move |input: ForwardedInput| {
                            log.update(|log| log.extend(flattened(input)))
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

    harness.down(pos2(50.0, 50.0), PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(pos2(300.0, 250.0))]);
    harness.up(pos2(300.0, 250.0), PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(pos2(310.0, 250.0))]);

    let log = log.get_untracked();
    assert!(log.contains(&Event::PointerMoved(pos2(300.0, 250.0))));
    assert!(log.iter().any(|event| matches!(
        event,
        Event::PointerButton { pressed: false, pos, .. } if *pos == pos2(300.0, 250.0)
    )));
    assert!(
        !log.contains(&Event::PointerMoved(pos2(310.0, 250.0))),
        "once released, a move outside is not the catcher's"
    );
}
