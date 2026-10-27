use super::*;
use block_editor_beui::beui::Pos2;

fn arrow_of(editor: &BeuiTest<CanvasApp>) -> CanvasEntity {
    entities(editor)
        .into_iter()
        .find(|entity| entity.shape == CanvasShape::Path)
        .expect("the arrow is on the canvas")
}

#[test]
fn dragging_the_middle_of_a_line_bends_it_at_a_new_point() {
    let mut from = card();
    from.style.stroke_width = 0.0;
    from.transform = CanvasTransform::new(
        CanvasPoint::new(-150.0, 0.0),
        CanvasPoint::new(100.0, 80.0),
        0.0,
    );
    let mut to = card();
    to.id = Uuid::from_u128(2);
    to.style.stroke_width = 0.0;
    to.transform = CanvasTransform::new(
        CanvasPoint::new(150.0, 0.0),
        CanvasPoint::new(100.0, 80.0),
        0.0,
    );
    let mut arrow = path(
        Uuid::from_u128(3),
        &[CanvasPoint::new(-96.0, 0.0), CanvasPoint::new(96.0, 0.0)],
    );
    arrow.style.end_arrowhead = CanvasArrowhead::Arrow;
    arrow.bindings = [
        Some(infinite_canvas_types::CanvasBinding {
            entity: from.id,
            anchor: CanvasPoint::default(),
        }),
        Some(infinite_canvas_types::CanvasBinding {
            entity: to.id,
            anchor: CanvasPoint::default(),
        }),
    ];
    let mut editor = vector_editor(&[from.clone(), to.clone(), arrow.clone()]);
    let anchor = editor.rect_of(&format!("infinite-canvas.entity.{}", from.id));
    let scale = anchor.width() / 104.0;
    let at = |x: f32, y: f32| -> Pos2 { anchor.center() + Vec2::new(x + 150.0, y) * scale };

    editor.click(at(40.0, 0.0));
    assert_eq!(editor.label("infinite-canvas.selection"), "Arrow selected");
    editor.snapshot("a_selected_arrow_shows_a_handle_at_its_middle");

    editor.drag(at(0.0, 0.0), at(0.0, 80.0));
    let bent = arrow_of(&editor);
    let world = bent.world_points();
    assert_eq!(world.len(), 3, "the drag put a new point in the middle");
    let corner = world[1].at();
    assert!(
        corner.x.abs() < 0.5 && (corner.y - 80.0).abs() < 0.5,
        "the new point is where the middle was dragged to, got {corner:?}"
    );
    assert_eq!(
        bent.bindings
            .map(|binding| binding.map(|binding| binding.entity)),
        [Some(from.id), Some(to.id)],
        "bending keeps both ends bound"
    );
    editor.snapshot("dragging_the_middle_of_a_line_bends_it_at_a_new_point");

    editor.drag(at(0.0, 80.0), at(0.0, -80.0));
    let corner = arrow_of(&editor).world_points()[1].at();
    assert!(
        (corner.y + 80.0).abs() < 0.5 && arrow_of(&editor).points.len() == 3,
        "the new point can be dragged again, got {corner:?}"
    );

    let mut moved = to.clone();
    moved.transform.center = CanvasPoint::new(150.0, 160.0);
    apply(
        &mut editor,
        InfiniteCanvasOperation::Update {
            entities: vec![moved],
        },
    );
    editor.run();
    let [_, head] = crate::geometry::path_ends(&arrow_of(&editor));
    assert!(
        head.y > 100.0,
        "the bound head still follows the shape it points at, got {head:?}"
    );
}
