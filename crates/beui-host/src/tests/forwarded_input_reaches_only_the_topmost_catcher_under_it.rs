use super::*;
use crate::ForwardedInput;
use crate::primitives::*;
use crate::reactive::*;

fn moves(log: ReadSignal<Vec<ForwardedInput>>) -> Vec<Pos2> {
    log.with_untracked(|log| {
        log.iter()
            .flat_map(|input| flattened(input.clone()))
            .filter_map(|event| match event {
                Event::PointerMoved(pos) => Some(pos),
                _ => None,
            })
            .collect()
    })
}

#[test]
fn forwarded_input_reaches_only_the_topmost_catcher_under_it() {
    let _root = crate::reactive::RootScope::new();
    let below = create_signal(Vec::<ForwardedInput>::new());
    let above = create_signal(Vec::<ForwardedInput>::new());
    let document = build_node(move || {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=200.0 height=200.0>
                    <Interactive on_forward={move |input| below.update(|below| below.push(input))}>
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=50.0 y=50.0 width=50.0 height=50.0>
                    <Interactive on_forward={move |input| above.update(|above| above.push(input))}>
                        <Frame />
                    </Interactive>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(vec![Input::PointerMoved(pos2(60.0, 60.0))]);
    assert_eq!(moves(above.read_only()), [pos2(60.0, 60.0)]);
    assert!(
        moves(below.read_only()).is_empty(),
        "the catcher beneath is covered"
    );
    assert!(
        above
            .get_untracked()
            .last()
            .is_some_and(|input| input.hovered)
    );

    harness.frame(vec![Input::PointerMoved(pos2(150.0, 150.0))]);
    assert_eq!(moves(below.read_only()), [pos2(150.0, 150.0)]);
    assert!(
        above
            .get_untracked()
            .last()
            .is_some_and(|input| !input.hovered),
        "the catcher the pointer left hears that it left"
    );
}
