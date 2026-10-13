use super::*;
use crate::reactive::{Align, Frame, Interactive, Layers, build, view};

#[test]
fn a_catcher_beneath_an_overlapping_one_hears_no_press() {
    let (below, above) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let document = build({
        let (below, above) = (below.clone(), above.clone());
        move || {
            view! {
                <Layers>
                    <Interactive on_click={move || below.set(below.get() + 1)}>
                        <Frame width=100.0 height=100.0 />
                    </Interactive>
                    <Frame align_horizontal=Align::End align_vertical=Align::Start>
                        <Interactive on_click={move || above.set(above.get() + 1)}>
                            <Frame width=50.0 height=50.0 />
                        </Interactive>
                    </Frame>
                </Layers>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(100.0, 100.0));
    harness.frame(Vec::new());

    harness.click(pos2(80.0, 20.0));
    assert_eq!(
        (below.get(), above.get()),
        (0, 1),
        "the catcher on top takes the press without capturing it"
    );

    harness.click(pos2(20.0, 80.0));
    assert_eq!(
        (below.get(), above.get()),
        (1, 1),
        "beside the one on top the press reaches the one beneath"
    );
}
