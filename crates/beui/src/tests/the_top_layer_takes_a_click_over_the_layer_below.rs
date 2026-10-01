use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::reactive::{Align, ClickCatcher, Frame, Layers, view};

#[test]
fn the_top_layer_takes_a_click_over_the_layer_below() {
    let (below, above) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let document = build({
        let (below, above) = (below.clone(), above.clone());
        move || {
            view! {
                <Layers>
                    <ClickCatcher
                        capture_presses=true
                        on_click={move || below.set(below.get() + 1)}
                    >
                        <Frame width=100.0 height=100.0 />
                    </ClickCatcher>
                    <Frame align_horizontal=Align::End align_vertical=Align::Start>
                        <ClickCatcher
                            capture_presses=true
                            on_click={move || above.set(above.get() + 1)}
                        >
                            <Frame width=20.0 height=20.0 />
                        </ClickCatcher>
                    </Frame>
                </Layers>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(100.0, 100.0));
    harness.frame(Vec::new());

    harness.click(pos2(90.0, 10.0));
    assert_eq!((below.get(), above.get()), (0, 1));

    harness.click(pos2(10.0, 90.0));
    assert_eq!((below.get(), above.get()), (1, 1));
}
