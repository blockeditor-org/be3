use super::*;
use crate::geometry::vec2;
use crate::reactive::*;
use crate::unstyled::{PanZoomApi, PanZoomView};

#[test]
fn scrolling_a_pan_zoom_pans_it() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(PanZoomView::IDENTITY);
    let document = build_node(move || {
        view! {
            <PanZoomStage view={reported} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let item = harness.find("item");
    assert_eq!(
        harness.rect(item),
        Rect::from_min_size(pos2(200.0, 150.0), Vec2::new(100.0, 50.0))
    );

    harness.scroll_at(pos2(200.0, 150.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());

    let view = harness
        .document()
        .api::<PanZoomApi>(harness.find("stage"))
        .view()
        .get();
    assert_eq!(view, PanZoomView::new(pos2(0.0, 20.0), 1.0));
    assert_eq!(reported.get_untracked(), view);
    assert_eq!(
        harness.rect(item),
        Rect::from_min_size(pos2(200.0, 130.0), Vec2::new(100.0, 50.0))
    );
}
