use super::*;
use block_editor_beui::beui::{Input, Pen, PenPhase, PointerButton, Pos2};

fn grid(count: usize) -> Vec<CanvasEntity> {
    (0..count)
        .map(|index| {
            let mut shape = entity(Uuid::from_u128(index as u128 + 1));
            shape.transform = CanvasTransform::new(
                CanvasPoint::new((index % 8) as f32 * 70.0 - 245.0, (index / 8) as f32 * 50.0),
                CanvasPoint::new(60.0, 40.0),
                0.0,
            );
            shape.style.fill = CanvasFill::Solid;
            shape
        })
        .collect()
}

fn pen(editor: &mut BeuiTest<CanvasApp>, phase: PenPhase, pos: Pos2) -> (usize, usize) {
    editor.step(vec![Input::Pen {
        phase,
        pos,
        pen: Pen {
            pressure: 0.5,
            ..Pen::default()
        },
    }]);
    let work = editor.document().performance().latest.work;
    (work.painted_nodes, work.replayed_nodes)
}

fn moved(count: usize) -> (usize, usize) {
    let mut editor = editor(&grid(count));
    editor.click("infinite-canvas.dock.tool.Pen");
    editor.settle();
    let from = editor.rect_of("infinite-canvas.canvas").center() - Vec2::new(100.0, 60.0);
    pen(&mut editor, PenPhase::Move, from);
    pen(&mut editor, PenPhase::Down(PointerButton::Primary), from);
    pen(&mut editor, PenPhase::Move, from + Vec2::new(20.0, 5.0));
    pen(&mut editor, PenPhase::Move, from + Vec2::new(40.0, 0.0))
}

#[test]
fn a_pen_move_paints_and_visits_no_entity_again() {
    assert_eq!(
        moved(6),
        moved(48),
        "a pen move painted or visited more nodes beside 48 entities than beside 6"
    );
}
