use super::*;
use crate::reactive::{create_signal, view, with_reactive_scope};
use crate::unstyled::PanZoomView;

#[test]
fn a_pan_zoom_follows_the_view_its_caller_sets() {
    let signal = Rc::new(RefCell::new(None));
    let sink = signal.clone();
    let document = build(move || {
        let (view, set_view) = create_signal(PanZoomView::IDENTITY);
        sink.replace(Some(set_view));
        view! {
            <PanZoomStage view on_change={move |_| {}} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let set_view = signal.borrow_mut().take().expect("the view was not kept");
    with_reactive_scope(harness.document_mut(), move || {
        set_view.set(PanZoomView::new(pos2(50.0, 25.0), 2.0));
    });
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(harness.find("item")),
        Rect::from_min_size(pos2(100.0, 100.0), Vec2::new(200.0, 100.0))
    );
}
