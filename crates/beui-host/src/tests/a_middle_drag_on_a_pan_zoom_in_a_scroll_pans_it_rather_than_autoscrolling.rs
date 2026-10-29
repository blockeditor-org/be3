use super::*;
use crate::FontId;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{PanZoomApi, PanZoomView, Scroll};

#[test]
fn a_middle_drag_on_a_pan_zoom_in_a_scroll_pans_it_rather_than_autoscrolling() {
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame height=200.0>
                        <PanZoomStage />
                    </Frame>
                    <ForEach keys={indices(20)}>|index: usize|
                        <Text color=Color32::WHITE font={FontId::proportional(20.0)}>
                            {format!("Row {index}")}
                        </Text>
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.middle_drag(pos2(200.0, 100.0), pos2(240.0, 130.0));
    harness.frame(Vec::new());

    assert!(harness.document().pointer_grabber().is_none());
    assert_eq!(
        harness
            .document()
            .api::<PanZoomApi>(harness.find("stage"))
            .view()
            .get(),
        PanZoomView::new(pos2(-40.0, -30.0), 1.0)
    );
}
