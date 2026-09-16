use super::*;
use crate::geometry::vec2;
use crate::reactive::view;
use crate::unstyled::{PanZoomView, pan_zoom_view};

#[test]
fn shift_scrolling_a_pan_zoom_pans_it_sideways() {
    let document = build(move || {
        view! {
            <PanZoomStage view=PanZoomView::IDENTITY on_change={move |_| {}} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.scroll(pos2(200.0, 150.0), vec2(0.0, -20.0), Modifiers::SHIFT);
    harness.frame(Vec::new());

    let view = pan_zoom_view(harness.document(), harness.find("stage")).get();
    assert_eq!(view, PanZoomView::new(pos2(20.0, 0.0), 1.0));
    assert_eq!(
        harness.rect(harness.find("item")),
        Rect::from_min_size(pos2(180.0, 150.0), Vec2::new(100.0, 50.0))
    );
}
