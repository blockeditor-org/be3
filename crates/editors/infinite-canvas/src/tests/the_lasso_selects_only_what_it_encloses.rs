use super::*;
use block_editor_beui::beui::{Input, Key, Modifiers, PointerButton, Pos2};

fn boxed(id: u128, x: f32, y: f32, width: f32, height: f32) -> CanvasEntity {
    let mut boxed = card();
    boxed.id = Uuid::from_u128(id);
    boxed.style.stroke_width = 0.0;
    boxed.transform =
        CanvasTransform::new(CanvasPoint::new(x, y), CanvasPoint::new(width, height), 0.0);
    boxed
}

#[test]
fn the_lasso_selects_only_what_it_encloses() {
    let left = boxed(1, -170.0, 0.0, 100.0, 60.0);
    let middle = boxed(2, 0.0, 0.0, 100.0, 60.0);
    let right = boxed(3, 170.0, 0.0, 100.0, 60.0);
    let crossing = boxed(4, 0.0, 70.0, 100.0, 30.0);
    let line = path(
        Uuid::from_u128(5),
        &[
            CanvasPoint::new(-200.0, 52.0),
            CanvasPoint::new(200.0, 52.0),
        ],
    );
    let mut editor = vector_editor(&[
        left.clone(),
        middle.clone(),
        right.clone(),
        crossing.clone(),
        line.clone(),
    ]);
    let anchor = editor.rect_of(&format!("infinite-canvas.entity.{}", middle.id));
    let scale = anchor.width() / 104.0;
    let at = |x: f32, y: f32| -> Pos2 { anchor.center() + Vec2::new(x, y) * scale };
    let lasso = [
        at(-240.0, -60.0),
        at(-240.0, 60.0),
        at(240.0, 60.0),
        at(240.0, -60.0),
        at(75.0, -60.0),
        at(75.0, 45.0),
        at(-75.0, 45.0),
        at(-75.0, -60.0),
        at(-200.0, -60.0),
    ];

    editor.click("infinite-canvas.dock.tool.Select");
    editor.step(vec![
        Input::PointerMoved(lasso[0]),
        Input::PointerButton {
            pos: lasso[0],
            button: PointerButton::Primary,
            pressed: true,
        },
    ]);
    for at in &lasso[1..] {
        editor.step(vec![Input::PointerMoved(*at)]);
    }
    editor.snapshot("the_lasso_is_drawn_while_it_is_dragged");
    editor.step(vec![Input::PointerButton {
        pos: lasso[lasso.len() - 1],
        button: PointerButton::Primary,
        pressed: false,
    }]);
    editor.run();

    assert_eq!(
        editor.label("infinite-canvas.selection"),
        "3 objects selected"
    );
    editor.snapshot("the_lasso_selects_only_what_it_encloses");

    editor.key(Key::Delete, Modifiers::NONE);
    let mut left_over: Vec<Uuid> = entities(&editor).iter().map(|entity| entity.id).collect();
    left_over.sort();
    assert_eq!(
        left_over,
        vec![middle.id, crossing.id],
        "the lasso took the two shapes and the line it went around, not the shape in its notch \
         or the one across its edge"
    );
}
