use super::*;
use crate::reactive::{Frame, NodeRef, build, view};

#[test]
fn recolouring_a_nested_frame_repaints_it_alone() {
    let (outer, inner) = (NodeRef::new(), NodeRef::new());
    let mut document = build({
        let (outer, inner) = (outer.clone(), inner.clone());
        move || {
            view! {
                <Frame
                    @node_ref=&outer
                    color=Color32::WHITE
                    radius=0
                    padding_horizontal=20.0
                    padding_vertical=20.0
                >
                    <Frame @node_ref=&inner height=40.0 color={Color32::from_gray(40)} radius=0 />
                </Frame>
            }
        }
    });
    let (outer, inner) = (outer.get(), inner.get());
    let (outer_layouts, outer_paints) = counted(&mut document, outer);
    let (inner_layouts, inner_paints) = counted(&mut document, inner);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness
        .document_mut()
        .set_frame_color(inner, Color32::from_gray(200));
    let output = harness.frame(Vec::new());

    assert_eq!(
        (outer_layouts.get(), inner_layouts.get()),
        (1, 1),
        "a new colour lays nothing out"
    );
    assert_eq!(
        (outer_paints.get(), inner_paints.get()),
        (1, 2),
        "the frame around the recoloured one keeps what it painted"
    );
    assert_eq!(
        output.damage.rects(),
        [Rect::from_min_size(
            harness.rect(inner).min,
            Vec2::new(harness.rect(inner).width(), 40.0)
        )],
        "only what the recoloured frame fills is damaged"
    );
}
