use super::*;
use crate::ForwardedInput;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_catcher_that_lost_its_release_lets_go_of_the_pointer() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal::<Vec<(usize, Event)>>(Vec::new());
    let catcher = |index: usize, heard: Signal<Vec<(usize, Event)>>| {
        move |input: ForwardedInput| {
            heard.update(|heard| {
                heard.extend(flattened(input).into_iter().map(|event| (index, event)))
            })
        }
    };
    let document = build_node(move || {
        let first = catcher(0, heard);
        let second = catcher(1, heard);
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                    <Interactive on_forward={first}>
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                    <Interactive on_forward={second}>
                        <Frame />
                    </Interactive>
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.press_at(pos2(50.0, 50.0));
    harness.frame(vec![Input::PointersCancelled]);
    heard.update(|heard| heard.clear());
    harness.frame(vec![Input::PointerMoved(pos2(250.0, 50.0))]);

    assert_eq!(
        heard.get_untracked(),
        [(1, Event::PointerMoved(pos2(250.0, 50.0)))],
        "with no button down any more, the move goes to the catcher under the pointer"
    );
}
