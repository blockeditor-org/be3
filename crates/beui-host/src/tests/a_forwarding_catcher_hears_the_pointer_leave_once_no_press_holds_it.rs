use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::{Forwarded, ForwardedInput};

fn lefts(heard: ReadSignal<Vec<Forwarded>>) -> usize {
    heard.with_untracked(|heard| {
        heard
            .iter()
            .filter(|event| **event == Forwarded::Left)
            .count()
    })
}

#[test]
fn a_forwarding_catcher_hears_the_pointer_leave_once_no_press_holds_it() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::<Forwarded>::new());
    let document = build_node(move || {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                    <Interactive
                        on_forward={move |input: ForwardedInput| {
                            heard.update(|heard| heard.extend(input.events))
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
    let (inside, outside) = (pos2(50.0, 50.0), pos2(250.0, 50.0));

    harness.frame(vec![Input::PointerMoved(inside)]);
    harness.frame(vec![Input::PointerMoved(outside)]);
    harness.frame(vec![Input::PointerMoved(pos2(260.0, 50.0))]);
    assert_eq!(
        lefts(heard.read_only()),
        1,
        "the catcher hears the pointer leave, once"
    );

    heard.update(|heard| heard.clear());
    harness.down(inside, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(outside)]);
    assert_eq!(
        lefts(heard.read_only()),
        0,
        "a press it holds keeps the pointer"
    );
    harness.frame(vec![Input::PointerButton {
        pos: outside,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    assert_eq!(lefts(heard.read_only()), 1, "until it is let go");
}
