use super::*;
use crate::geometry::vec2;
use crate::reactive::view;
use crate::unstyled::{PanZoomView, pan_zoom_view};

#[test]
fn scrolling_a_pan_zoom_pans_it() {
    let reported = Rc::new(Cell::new(None));
    let sink = reported.clone();
    let document = build(move || {
        view! {
            <PanZoomStage view=PanZoomView::IDENTITY on_change={move |view| sink.set(Some(view))} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let item = harness.find("item");
    assert_eq!(
        harness.rect(item),
        Rect::from_min_size(pos2(200.0, 150.0), Vec2::new(100.0, 50.0))
    );

    harness.scroll(pos2(200.0, 150.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());

    let view = pan_zoom_view(harness.document(), harness.find("stage")).get();
    assert_eq!(view, PanZoomView::new(pos2(0.0, 20.0), 1.0));
    assert_eq!(reported.get(), Some(view));
    assert_eq!(
        harness.rect(item),
        Rect::from_min_size(pos2(200.0, 130.0), Vec2::new(100.0, 50.0))
    );
}
