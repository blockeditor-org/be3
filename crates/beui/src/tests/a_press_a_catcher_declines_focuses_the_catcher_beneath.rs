use super::*;
use crate::ForwardedInput;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_press_a_catcher_declines_focuses_the_catcher_beneath() {
    let _root = crate::reactive::RootScope::new();
    let below = create_signal(Vec::<Event>::new());
    let above = create_signal(Vec::<Event>::new());
    let document = build_node(move || {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                    <Interactive
                        focusable=true
                        on_forward={move |input: ForwardedInput| {
                            if input.focused {
                                below.update(|below| below.extend(flattened(input)));
                            }
                        }}
                    >
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                    <Interactive
                        focusable=true
                        hit_at={|local: Pos2| local.x >= 100.0}
                        on_forward={move |input: ForwardedInput| {
                            if input.focused {
                                above.update(|above| above.extend(flattened(input)));
                            }
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

    harness.click(pos2(50.0, 50.0));
    below.update(|below| below.clear());
    above.update(|above| above.clear());
    harness.frame(vec![Input::Text("a".to_owned())]);
    assert_eq!(below.get_untracked(), [Event::Text("a".to_owned())]);
    assert!(
        above.with_untracked(|above| above.is_empty()),
        "the catcher that declined the press is not focused"
    );

    harness.click(pos2(150.0, 50.0));
    below.update(|below| below.clear());
    above.update(|above| above.clear());
    harness.frame(vec![Input::Text("b".to_owned())]);
    assert_eq!(above.get_untracked(), [Event::Text("b".to_owned())]);
    assert!(below.with_untracked(|below| below.is_empty()));
}
