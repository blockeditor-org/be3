use super::*;
use crate::reactive::{Frame, view};
use crate::screen_simulation::ScreenSimulation;
use crate::styled::Checkbox;

#[test]
fn a_simulated_screen_larger_than_the_window_is_shrunk_to_fit() {
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
    harness
        .context
        .set_screen_simulation(Some(ScreenSimulation {
            size: WIDE_VIEWPORT * 2.0,
            zoom: None,
        }));
    let output = harness.frame(Vec::new());

    harness.assert_screen_size(WIDE_VIEWPORT * 2.0);
    let scale = harness.document().screen_scale();
    assert!(scale < 0.5);
    let laid_out = harness.rect(checkbox);
    let shown = output.test_id_rect("option").expect("the option was shown");
    assert!((shown.width() - laid_out.width() * scale).abs() < 0.01);

    let target = shown.center();
    harness.click(target);

    assert!(styled::checkbox_checked(harness.document(), checkbox));
}
