use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_catcher_beneath_an_overlapping_one_hears_no_press() {
    let _root = crate::reactive::RootScope::new();
    let below = create_signal(0);
    let above = create_signal(0);
    let document = build_node(move || {
        view! {
            <Layers>
                <Interactive on_click={move || below.update(|below| *below += 1)}>
                    <Frame width=100.0 height=100.0 />
                </Interactive>
                <Frame align={Align2::new(Align::End, Align::Start)}>
                    <Interactive on_click={move || above.update(|above| *above += 1)}>
                        <Frame width=50.0 height=50.0 />
                    </Interactive>
                </Frame>
            </Layers>
        }
    });
    let mut harness = Harness::sized(document, vec2(100.0, 100.0));
    harness.frame(Vec::new());

    harness.click(pos2(80.0, 20.0));
    assert_eq!(
        (below.get_untracked(), above.get_untracked()),
        (0, 1),
        "the catcher on top takes the press without capturing it"
    );

    harness.click(pos2(20.0, 80.0));
    assert_eq!(
        (below.get_untracked(), above.get_untracked()),
        (1, 1),
        "beside the one on top the press reaches the one beneath"
    );
}
