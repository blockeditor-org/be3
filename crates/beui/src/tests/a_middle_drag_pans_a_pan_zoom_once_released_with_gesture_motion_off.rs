use super::*;
use crate::Motion;
use crate::reactive::view;
use crate::unstyled::{PanZoomView, pan_zoom_view};

#[test]
fn a_middle_drag_pans_a_pan_zoom_once_released_with_gesture_motion_off() {
    let document = build(move || {
        view! {
            <PanZoomStage view=PanZoomView::IDENTITY on_change={move |_| {}} />
        }
    });
    let mut harness = Harness::new(document);
    harness.context().set_motion(Motion::Still);
    harness.frame(Vec::new());
    let view = |harness: &Harness| pan_zoom_view(harness.document(), harness.find("stage")).get();

    harness.frame(vec![Event::PointerMoved(pos2(200.0, 150.0))]);
    harness.middle_button(pos2(200.0, 150.0), true);
    harness.frame(vec![Event::PointerMoved(pos2(220.0, 165.0))]);
    harness.frame(vec![Event::PointerMoved(pos2(240.0, 180.0))]);

    assert_eq!(
        view(&harness),
        PanZoomView::IDENTITY,
        "the view holds still while the button is down"
    );

    harness.middle_button(pos2(240.0, 180.0), false);
    harness.frame(Vec::new());

    assert_eq!(view(&harness), PanZoomView::new(pos2(-40.0, -30.0), 1.0));
}
