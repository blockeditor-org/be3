use super::*;
use infinite_canvas_types::{BINDING_GAP, CanvasBinding};

#[test]
fn a_selected_bound_arrow_shows_where_each_end_is_anchored() {
    let mut from = card();
    from.transform = CanvasTransform::new(
        CanvasPoint::new(-150.0, 0.0),
        CanvasPoint::new(100.0, 80.0),
        0.0,
    );
    let mut to = shaped(Uuid::from_u128(2), CanvasShape::Ellipse);
    to.transform = CanvasTransform::new(
        CanvasPoint::new(150.0, 40.0),
        CanvasPoint::new(120.0, 100.0),
        0.0,
    );
    let mut arrow = path(
        Uuid::from_u128(3),
        &[
            CanvasPoint::new(-130.0, 10.0),
            CanvasPoint::new(140.0, 20.0),
        ],
    );
    arrow.style.end_arrowhead = CanvasArrowhead::Arrow;
    let anchors = [CanvasPoint::new(0.2, 0.125), CanvasPoint::new(-0.1, -0.2)];
    arrow.bindings = [
        Some(CanvasBinding {
            entity: from.id,
            anchor: anchors[0],
        }),
        Some(CanvasBinding {
            entity: to.id,
            anchor: anchors[1],
        }),
    ];
    let mut editor = vector_editor(&[from.clone(), to.clone(), arrow.clone()]);

    let [tail, head] = crate::geometry::path_ends(
        &entities(&editor)
            .into_iter()
            .find(|entity| entity.id == arrow.id)
            .expect("the arrow is on the canvas"),
    );
    let outside = |shape: &CanvasEntity, at: CanvasPoint| {
        !shape.shape.contains_local(shape.transform.to_local(at))
    };
    assert!(
        outside(&from, tail) && outside(&to, head),
        "the ends stop short of the shapes they are anchored in, got {tail:?} and {head:?}"
    );
    assert!(
        (tail.x - (-100.0 + BINDING_GAP)).abs() < 0.5,
        "the tail stops a gap short of the rectangle's right edge, got {tail:?}"
    );

    editor.click(format!("infinite-canvas.entity.{}", arrow.id));
    editor.run();
    assert_eq!(editor.label("infinite-canvas.selection"), "Arrow selected");
    editor.snapshot("a_selected_bound_arrow_shows_where_each_end_is_anchored");
}
