use super::*;
use crate::reactive::{Frame, view};
use crate::screen_simulation::ScreenSimulation;
use crate::styled::Checkbox;

#[test]
fn zooming_into_the_simulated_screen_follows_the_pointer() {
    let (document, [_, checkbox]) = toolbar_of(|| {
        [
            view! {
                <Frame width=400.0 height=300.0 />
            },
            view! {
                <Checkbox @test_id={"option"} label="Far option" checked=false />
            },
        ]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.context.set_screen_simulation(ScreenSimulation {
        size: 1.0,
        zoom: Some(2.0),
    });
    harness.frame(Vec::new());

    let root = harness.document().root().expect("the toolbar was built");
    let target = harness.center(checkbox) - harness.rect(root).min.to_vec2();
    let output = harness.frame(vec![Event::PointerMoved(target)]);

    let laid_out = harness.rect(checkbox);
    let shown = output.test_id_rect("option").expect("the option was shown");
    assert!((shown.width() - laid_out.width() * 2.0).abs() < 0.01);
    assert!(shown.contains(target), "the pointer is not over the option");

    harness.click(target);

    assert!(styled::checkbox_checked(harness.document(), checkbox));
}
