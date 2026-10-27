use super::*;
use block_editor_beui::beui::{Modifiers, Pos2};
use infinite_canvas_types::CanvasBinding;

use crate::app::canvas::item_bounds;

fn drawn_lines(editor: &BeuiTest<CanvasApp>, before: &[Uuid]) -> Vec<CanvasEntity> {
    entities(editor)
        .into_iter()
        .filter(|entity| entity.shape == CanvasShape::Path && !before.contains(&entity.id))
        .collect()
}

#[test]
fn with_binding_off_or_ctrl_held_a_line_drawn_onto_a_shape_stays_free() {
    let mut rectangle = card();
    rectangle.transform = CanvasTransform::new(
        CanvasPoint::new(150.0, 0.0),
        CanvasPoint::new(100.0, 160.0),
        0.0,
    );
    let mut bound = path(
        Uuid::from_u128(3),
        &[CanvasPoint::new(-150.0, 60.0), CanvasPoint::new(96.0, 60.0)],
    );
    bound.bindings[1] = Some(CanvasBinding {
        entity: rectangle.id,
        anchor: CanvasPoint::new(0.0, 0.375),
    });
    let mut editor = editor(&[rectangle.clone(), bound.clone()]);
    let shown = editor.rect_of(&format!("infinite-canvas.entity.{}", rectangle.id));
    let scale = shown.width() / item_bounds(&rectangle).width();
    let origin = shown.center() - Vec2::new(150.0, 0.0) * scale;
    let at = |x: f32, y: f32| -> Pos2 { origin + Vec2::new(x, y) * scale };

    editor.click("infinite-canvas.dock.tool.Line");
    editor.click("infinite-canvas.bind-lines");
    editor.drag(at(-150.0, -60.0), at(160.0, -60.0));
    let [free] = &drawn_lines(&editor, &[bound.id])[..] else {
        panic!("the drag drew one line");
    };
    assert_eq!(
        free.bindings,
        [None, None],
        "binding is off, so the line stays free"
    );
    let end = free.world_points()[1].at();
    assert!(
        (end.x - 160.0).abs() < 0.5,
        "the free end stays where it was drawn, inside the rectangle, got {end:?}"
    );
    assert_eq!(
        entities(&editor)
            .into_iter()
            .find(|entity| entity.id == bound.id)
            .map(|entity| entity.bindings),
        Some(bound.bindings),
        "turning binding off leaves the lines already bound as they were"
    );
    editor.click("infinite-canvas.dock.tool.Line");
    editor.snapshot("with_binding_off_a_line_drawn_onto_a_shape_stays_free");

    editor.click("infinite-canvas.bind-lines");
    editor.hold_modifiers(Modifiers::CTRL);
    editor.drag(at(-150.0, 0.0), at(160.0, 0.0));
    editor.hold_modifiers(Modifiers::NONE);
    let drawn = drawn_lines(&editor, &[bound.id, free.id]);
    let [held] = &drawn[..] else {
        panic!("the second drag drew one line");
    };
    assert_eq!(
        held.bindings,
        [None, None],
        "holding Ctrl while drawing keeps the line free with binding on"
    );
}
