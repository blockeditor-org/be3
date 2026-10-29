use super::*;
use crate::ForwardedInput;
use crate::HitRegion;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_catcher_that_declines_a_press_declines_the_embed_inside_it() {
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
                    <Interactive>
                        <Interactive
                            focusable=true
                            hit_area={HitRegion::rect(Rect::from_min_max(pos2(100.0, 0.0), pos2(200.0, 200.0)))}
                            on_forward={move |input: ForwardedInput| {
                                if input.focused {
                                    above.update(|above| above.extend(flattened(input)));
                                }
                            }}
                        >
                            <Embed slot={EmbedSlot::new()} punch=false />
                        </Interactive>
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
    assert_eq!(
        below.get_untracked(),
        [Event::Text("a".to_owned())],
        "a press the catcher declines passes its embed and the bare wrapper around it to the catcher beneath"
    );
    assert!(above.with_untracked(|above| above.is_empty()));

    harness.click(pos2(150.0, 50.0));
    below.update(|below| below.clear());
    above.update(|above| above.clear());
    harness.frame(vec![Input::Text("b".to_owned())]);
    assert_eq!(above.get_untracked(), [Event::Text("b".to_owned())]);
    assert!(below.with_untracked(|below| below.is_empty()));
}
