use super::*;
use crate::geometry::vec2;
use crate::reactive::{ItemSize, Scroll, view};
use crate::unstyled::{PanZoomView, pan_zoom_view};

#[test]
fn scrolling_a_pan_zoom_leaves_the_scroll_around_it_alone() {
    let reported = Rc::new(Cell::new(None));
    let sink = reported.clone();
    let document = build(move || {
        let mut items = vec![view! {
            <Frame height=200.0>
                <PanZoomStage view=PanZoomView::IDENTITY on_change={move |_| {}} />
            </Frame>
        }];
        items.extend((0..20).map(|index| {
            view! {
                <Text string={format!("Row {index}")} font_size=20.0 color=Color32::WHITE />
            }
        }));
        view! {
            <Column spacing=0.0>
                <Scroll
                    @sizing=ItemSize::Percent(100.0)
                    on_change={move |position: crate::base::ScrollPosition| {
                        sink.set(Some(position.offset));
                    }}
                    children={items}
                />
            </Column>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.scroll(pos2(200.0, 100.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());

    let view = pan_zoom_view(harness.document(), harness.find("stage")).get();
    assert_eq!(view, PanZoomView::new(pos2(0.0, 20.0), 1.0));
    assert_eq!(reported.get(), Some(0.0));

    harness.scroll(pos2(200.0, 280.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(reported.get(), Some(20.0));
    assert_eq!(
        pan_zoom_view(harness.document(), harness.find("stage")).get(),
        view
    );
}
