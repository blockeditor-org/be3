use super::*;
use std::rc::Rc;

#[test]
fn a_drawing_is_a_pink_box_of_its_size() {
    painting([200, 120], || {
        let draw: Draw = Rc::new(|_, _| {});
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START} padding={Sides::all(10.0)}>
                    <Drawing draw={draw} size={Some(vec2(80.0, 50.0))} />
                </Frame>
            </Root>
        }
    })
    .snapshot("html_drawing.browser");
}
