use super::*;
use crate::ForwardedInput;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_hovered_catcher_hears_where_the_pointer_is_while_another_holds_it() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::<(bool, Option<Pos2>, usize)>::new());
    let document = build_node(move || {
        view! {
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=100.0>
                    <Interactive on_forward={|_input: ForwardedInput| {}}>
                        <Frame />
                    </Interactive>
                </CanvasItem>
                <CanvasItem x=200.0 y=0.0 width=100.0 height=100.0>
                    <Interactive
                        on_forward={move |input: ForwardedInput| {
                            heard.update(|heard| heard.push((input.hovered, input.pointer, flattened(input.clone()).len())))
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

    harness.press_at(pos2(50.0, 50.0));
    harness.frame(vec![Input::PointerMoved(pos2(240.0, 50.0))]);
    harness.frame(vec![Input::PointerMoved(pos2(260.0, 60.0))]);

    let heard = heard.get_untracked();
    assert!(
        heard.contains(&(true, Some(pos2(260.0, 60.0)), 0)),
        "the catcher under the pointer hears each move without the events, {heard:?}"
    );
}
