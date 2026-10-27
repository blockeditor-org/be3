use super::*;
use block_editor_beui::beui::Vec2;

fn arrow_of(editor: &BeuiTest<CanvasApp>) -> CanvasEntity {
    entities(editor)
        .into_iter()
        .find(|entity| entity.shape == CanvasShape::Path)
        .expect("the drag drew an arrow")
}

fn ends(editor: &BeuiTest<CanvasApp>) -> [CanvasPoint; 2] {
    crate::geometry::path_ends(&arrow_of(editor))
}

#[test]
fn an_arrow_drawn_between_two_shapes_follows_them_when_they_move() {
    let mut from = entity(Uuid::from_u128(1));
    from.transform = CanvasTransform::new(
        CanvasPoint::new(-150.0, 0.0),
        CanvasPoint::new(100.0, 80.0),
        0.0,
    );
    from.style.fill = CanvasFill::Solid;
    from.style.fill_color = CanvasColor::Rgba {
        red: 250,
        green: 200,
        blue: 80,
        alpha: 255,
    };
    let mut to = shaped(Uuid::from_u128(2), CanvasShape::Ellipse);
    to.transform = CanvasTransform::new(
        CanvasPoint::new(150.0, 0.0),
        CanvasPoint::new(100.0, 80.0),
        0.0,
    );
    let mut editor = editor(&[from.clone(), to.clone()]);
    let start = editor
        .rect_of(&format!("infinite-canvas.entity.{}", from.id))
        .center();
    let end = editor
        .rect_of(&format!("infinite-canvas.entity.{}", to.id))
        .center();

    editor.click("infinite-canvas.dock.tool.Arrow");
    editor.drag(start, end);

    let arrow = arrow_of(&editor);
    assert_eq!(arrow.style.end_arrowhead, CanvasArrowhead::Arrow);
    assert_eq!(
        arrow
            .bindings
            .map(|binding| binding.map(|binding| binding.entity)),
        [Some(from.id), Some(to.id)],
        "each end of the arrow is bound to the shape it was drawn from or to"
    );
    let [tail, head] = ends(&editor);
    assert!(
        (tail.x + 96.0).abs() < 0.5 && (head.x - 96.0).abs() < 0.5,
        "the ends stop short of the shapes' outlines, got {tail:?} and {head:?}"
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
    let [_, head] = ends(&editor);
    assert!(
        head.y > 100.0 && head.x < 150.0,
        "the head follows the ellipse a peer moved, got {head:?}"
    );
    editor.snapshot("an_arrow_drawn_between_two_shapes_follows_them_when_they_move");

    editor.click("infinite-canvas.dock.tool.Select");
    editor.drag(start, start + Vec2::new(0.0, -120.0));
    let [tail, _] = ends(&editor);
    assert!(
        tail.y < -60.0,
        "the tail follows the rectangle dragged up, got {tail:?}"
    );
    assert_eq!(
        arrow_of(&editor)
            .bindings
            .map(|binding| binding.map(|binding| binding.entity)),
        [Some(from.id), Some(to.id)],
        "moving a shape keeps the arrow bound to it"
    );
}
