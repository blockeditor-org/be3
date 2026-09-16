use super::*;
use crate::reactive::view;
use crate::unstyled::{PanZoomView, pan_zoom_view};

#[test]
fn dragging_a_pan_zoom_with_the_middle_button_pans_it() {
    let document = build(move || {
        view! {
            <PanZoomStage view=PanZoomView::IDENTITY on_change={move |_| {}} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.middle_drag(pos2(200.0, 150.0), pos2(240.0, 180.0));
    harness.frame(Vec::new());

    let view = pan_zoom_view(harness.document(), harness.find("stage")).get();
    assert_eq!(view, PanZoomView::new(pos2(-40.0, -30.0), 1.0));
    assert_eq!(
        harness.rect(harness.find("item")),
        Rect::from_min_size(pos2(240.0, 180.0), Vec2::new(100.0, 50.0))
    );
}
