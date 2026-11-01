use super::*;
use crate::FontId;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{PanZoomApi, PanZoomView, Scroll};

#[test]
fn scrolling_a_pan_zoom_leaves_the_scroll_around_it_alone() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(None);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll
                    @sizing=ItemSize::Percent(100.0)
                    on_change={move |position: beui::ScrollPosition| {
                        reported.set(Some(position.offset));
                    }}
                >
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

    harness.scroll_at(pos2(200.0, 100.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());

    let view = harness
        .document()
        .api::<PanZoomApi>(harness.find("stage"))
        .view()
        .get();
    assert_eq!(view, PanZoomView::new(pos2(0.0, 20.0), 1.0));
    assert_eq!(reported.get_untracked(), Some(0.0));

    harness.scroll_at(pos2(200.0, 280.0), vec2(0.0, -20.0), Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(reported.get_untracked(), Some(20.0));
    assert_eq!(
        harness
            .document()
            .api::<PanZoomApi>(harness.find("stage"))
            .view()
            .get(),
        view
    );
}
