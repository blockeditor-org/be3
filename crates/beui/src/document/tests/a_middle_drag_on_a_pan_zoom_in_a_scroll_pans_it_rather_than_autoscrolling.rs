use super::*;
use crate::reactive::{ForEach, ItemSize, List, view};
use crate::unstyled::{PanZoomView, Scroll, pan_zoom_view};

#[test]
fn a_middle_drag_on_a_pan_zoom_in_a_scroll_pans_it_rather_than_autoscrolling() {
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame height=200.0>
                        <PanZoomStage view=PanZoomView::IDENTITY on_change={move |_| {}} />
                    </Frame>
                    <ForEach keys={indices(20)}>
                        {|index: usize| view! {
                            <Text
                                string={format!("Row {index}")}
                                font_size=20.0
                                color=Color32::WHITE
                            />
                        }}
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.middle_drag(pos2(200.0, 100.0), pos2(240.0, 130.0));
    harness.frame(Vec::new());

    assert!(harness.document().autoscroll.is_none());
    assert_eq!(
        pan_zoom_view(harness.document(), harness.find("stage")).get(),
        PanZoomView::new(pos2(-40.0, -30.0), 1.0)
    );
}
