use super::*;
use crate::reactive::{Frame, view};
use crate::screen_simulation::ScreenSimulation;
use crate::styled::Checkbox;

#[test]
fn a_simulated_larger_screen_is_laid_out_bigger_and_shrunk_to_fit() {
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
        size: 2.0,
        zoom: None,
    });
    let output = harness.frame(Vec::new());

    let laid_out = harness.rect(checkbox);
    let shown = output.test_id_rect("option").expect("the option was shown");
    assert!((shown.width() - laid_out.width() / 2.0).abs() < 0.01);

    let target = shown.center();
    harness.click(target);

    assert!(styled::checkbox_checked(harness.document(), checkbox));
}
