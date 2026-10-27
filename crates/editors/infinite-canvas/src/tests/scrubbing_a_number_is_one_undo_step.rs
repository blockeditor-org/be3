use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn scrubbing_a_number_is_one_undo_step() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(120.0, 80.0), 0.0);
    rectangle.style.fill = CanvasFill::Solid;
    let mut editor = vector_editor(std::slice::from_ref(&rectangle));
    editor.click(format!("infinite-canvas.entity.{}", rectangle.id));

    let field = editor.point_of("infinite-canvas.transform.x");
    let path: Vec<_> = (0..=6)
        .map(|step| field + Vec2::new(step as f32 * 10.0, 0.0))
        .collect();
    editor.down(path[0], PointerButton::Primary);
    for &at in &path[1..] {
        editor.advance(std::time::Duration::from_secs(1));
        editor.hover(at);
    }
    editor.up(path[6], PointerButton::Primary);
    assert_eq!(entities(&editor)[0].transform.center.x, 60.0);

    undo(&mut editor);
    assert_eq!(
        entities(&editor)[0].transform.center.x,
        0.0,
        "one undo takes back the whole scrub, however long it took"
    );
}
