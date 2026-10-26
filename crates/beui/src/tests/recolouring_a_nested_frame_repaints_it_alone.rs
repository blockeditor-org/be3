use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn recolouring_a_nested_frame_repaints_it_alone() {
    let _root = crate::reactive::RootScope::new();
    let (outer, inner) = (NodeRef::new(), NodeRef::new());
    let mut document = build_node(move || {
        view! {
            <Frame @node_ref=&outer fill=Color32::WHITE radius=0.0 padding=20.0>
                <Frame @node_ref=&inner height=40.0 fill={Color32::from_gray(40)} radius=0.0 />
            </Frame>
        }
    });
    let (outer, inner) = (outer.get(), kind_of::<FrameNode>(&document, inner.get()));
    let (outer_layouts, outer_paints) = counted(&mut document, outer);
    let (inner_layouts, inner_paints) = counted(&mut document, inner);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness
        .document_mut()
        .set_frame_color(inner, Color32::from_gray(200));
    let output = harness.frame(Vec::new());

    assert_eq!(
        (outer_layouts.get_untracked(), inner_layouts.get_untracked()),
        (1, 1),
        "a new colour lays nothing out"
    );
    assert_eq!(
        (outer_paints.get_untracked(), inner_paints.get_untracked()),
        (1, 2),
        "the frame around the recoloured one keeps what it painted"
    );
    assert_eq!(
        output.main().damage.rects(),
        [Rect::from_min_size(
            harness.rect(inner).min,
            Vec2::new(harness.rect(inner).width(), 40.0)
        )],
        "only what the recoloured frame fills is damaged"
    );
}
