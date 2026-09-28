use super::*;
use crate::reactive::view;
use crate::unstyled::{PanZoomView, pan_zoom_view};

#[test]
fn plus_and_minus_zoom_a_focused_pan_zoom_and_zero_resets_the_scale() {
    let document = build(move || {
        view! {
            <PanZoomStage view=PanZoomView::IDENTITY on_change={move |_| {}} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let stage = harness.find("stage");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Plus, Modifiers::NONE);
    harness.frame(Vec::new());
    let zoomed = pan_zoom_view(harness.document(), stage).get();
    assert!(
        (zoomed.scale - 1.25).abs() < 0.001,
        "the view reached {}",
        zoomed.scale
    );
    assert_eq!(zoomed.center, pos2(0.0, 0.0));

    harness.key(Key::Minus, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!((pan_zoom_view(harness.document(), stage).get().scale - 1.0).abs() < 0.001);

    harness.key(Key::Plus, Modifiers::NONE);
    harness.key(Key::Plus, Modifiers::NONE);
    harness.key(Key::Zero, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(pan_zoom_view(harness.document(), stage).get().scale, 1.0);
}
