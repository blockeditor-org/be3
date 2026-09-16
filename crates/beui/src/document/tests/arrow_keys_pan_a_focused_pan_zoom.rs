use super::*;
use crate::reactive::view;
use crate::unstyled::{PanZoomView, pan_zoom_view};

#[test]
fn arrow_keys_pan_a_focused_pan_zoom() {
    let document = build(move || {
        view! {
            <PanZoomStage view=PanZoomView::IDENTITY on_change={move |_| {}} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let stage = harness.find("stage");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        pan_zoom_view(harness.document(), stage).get(),
        PanZoomView::new(pos2(40.0, 40.0), 1.0)
    );
    assert_eq!(
        harness.rect(harness.find("item")),
        Rect::from_min_size(pos2(160.0, 110.0), Vec2::new(100.0, 50.0))
    );
}
