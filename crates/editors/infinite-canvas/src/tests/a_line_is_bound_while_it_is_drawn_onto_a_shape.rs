use super::*;
use block_editor_beui::beui::{Input, Pos2};
use infinite_canvas_types::BINDING_GAP;

use crate::app::canvas::item_bounds;
use crate::geometry::path_ends;

fn line_of(editor: &BeuiTest<CanvasApp>) -> CanvasEntity {
    entities(editor)
        .into_iter()
        .find(|entity| entity.shape == CanvasShape::Path)
        .expect("the line is on the canvas")
}

fn bound_to(line: &CanvasEntity) -> [Option<Uuid>; 2] {
    line.bindings
        .map(|binding| binding.map(|binding| binding.entity))
}

#[test]
fn a_line_is_bound_while_it_is_drawn_onto_a_shape() {
    let mut rectangle = card();
    rectangle.transform = CanvasTransform::new(
        CanvasPoint::new(150.0, 0.0),
        CanvasPoint::new(100.0, 80.0),
        0.0,
    );
    let mut ellipse = shaped(Uuid::from_u128(2), CanvasShape::Ellipse);
    ellipse.transform = CanvasTransform::new(
        CanvasPoint::new(150.0, 160.0),
        CanvasPoint::new(100.0, 80.0),
        0.0,
    );
    let mut editor = vector_editor(&[rectangle.clone(), ellipse.clone()]);
    let shown = editor.rect_of(&format!("infinite-canvas.entity.{}", rectangle.id));
    let scale = shown.width() / item_bounds(&rectangle).width();
    let origin = shown.center() - Vec2::new(150.0, 0.0) * scale;
    let at = |x: f32, y: f32| -> Pos2 { origin + Vec2::new(x, y) * scale };

    editor.click("infinite-canvas.dock.tool.Arrow");
    editor.run();
    editor.down(at(-150.0, 0.0), PointerButton::Primary);
    editor.run();
    editor.step(vec![Input::PointerMoved(at(0.0, 0.0))]);
    assert_eq!(
        bound_to(&line_of(&editor)),
        [None, None],
        "the arrow is free while its end is over empty canvas"
    );
    editor.step(vec![Input::PointerMoved(at(160.0, 10.0))]);
    editor.run();
    let drawing = line_of(&editor);
    assert_eq!(
        bound_to(&drawing),
        [None, Some(rectangle.id)],
        "the arrow binds to the rectangle as soon as its end is over it"
    );
    let [_, head] = path_ends(&drawing);
    assert!(
        (head.x - (100.0 - BINDING_GAP)).abs() < 0.5,
        "while drawing, the head already stops a gap short of the rectangle, got {head:?}"
    );
    editor.snapshot("a_line_is_bound_while_it_is_drawn_onto_a_shape");

    editor.up(at(160.0, 10.0), PointerButton::Primary);
    editor.run();
    let landed = line_of(&editor);
    assert_eq!(
        (bound_to(&landed), path_ends(&landed)),
        (bound_to(&drawing), path_ends(&drawing)),
        "the arrow lands bound as it was drawn"
    );

    editor.down(at(head.x, head.y), PointerButton::Primary);
    editor.run();
    editor.step(vec![Input::PointerMoved(at(140.0, 150.0))]);
    editor.run();
    let dragging = line_of(&editor);
    assert_eq!(
        bound_to(&dragging),
        [None, Some(ellipse.id)],
        "dragging the head onto the ellipse binds it there while the pointer moves"
    );
    let [_, moved] = path_ends(&dragging);
    assert!(
        !ellipse
            .shape
            .contains_local(ellipse.transform.to_local(moved)),
        "the dragged head stops short of the ellipse, got {moved:?}"
    );
    editor.up(at(140.0, 150.0), PointerButton::Primary);
    editor.run();
    assert_eq!(
        path_ends(&line_of(&editor)),
        path_ends(&dragging),
        "the head lands where it was shown while dragged"
    );
}
