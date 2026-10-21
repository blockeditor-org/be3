use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn the_top_layer_takes_a_click_over_the_layer_below() {
    let _root = crate::reactive::RootScope::new();
    let below = create_signal(0);
    let above = create_signal(0);
    let document = build_node(move || {
        view! {
            <Layers>
                <Interactive
                    capture_presses=true
                    on_click={move || below.update(|below| *below += 1)}
                >
                    <Frame width=100.0 height=100.0 />
                </Interactive>
                <Frame align={Align2::new(Align::End, Align::Start)}>
                    <Interactive
                        capture_presses=true
                        on_click={move || above.update(|above| *above += 1)}
                    >
                        <Frame width=20.0 height=20.0 />
                    </Interactive>
                </Frame>
            </Layers>
        }
    });
    let mut harness = Harness::sized(document, vec2(100.0, 100.0));
    harness.frame(Vec::new());

    harness.click(pos2(90.0, 10.0));
    assert_eq!((below.get_untracked(), above.get_untracked()), (0, 1));

    harness.click(pos2(10.0, 90.0));
    assert_eq!((below.get_untracked(), above.get_untracked()), (1, 1));
}
