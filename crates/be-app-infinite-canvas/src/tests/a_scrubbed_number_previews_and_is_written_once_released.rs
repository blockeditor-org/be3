use super::*;
use be_sdk_beui::beui::Vec2;

#[test]
fn a_scrubbed_number_previews_and_is_written_once_released() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(120.0, 80.0), 0.0);
    rectangle.style.fill = CanvasFill::Solid;
    let mut editor = vector_editor(std::slice::from_ref(&rectangle));
    let drawn = format!("infinite-canvas.entity.{}", rectangle.id);
    editor.click(&drawn);
    editor.reveal("infinite-canvas.transform.x");
    let resting = editor.rect_of(&drawn);
    editor.store().take_written(None);

    let field = editor.point_of("infinite-canvas.transform.x");
    let path: Vec<_> = (0..=6)
        .map(|step| field + Vec2::new(step as f32 * 10.0, 0.0))
        .collect();
    editor.down(path[0], PointerButton::Primary);
    for &at in &path[1..] {
        editor.advance(std::time::Duration::from_secs(1));
        editor.hover(at);
    }
    assert_eq!(
        editor.rect_of(&drawn).center().x - resting.center().x,
        60.0,
        "the scrub previews on the canvas from the block itself"
    );
    assert!(
        editor.store().take_written(None).is_empty(),
        "a half-scrubbed number is not one anyone chose, so nothing is written yet"
    );
    editor.up(path[6], PointerButton::Primary);
    assert_eq!(entities(&editor)[0].transform.center.x, 60.0);
    assert!(
        !editor.store().take_written(None).is_empty(),
        "the release writes the scrub"
    );

    undo(&mut editor);
    assert_eq!(
        entities(&editor)[0].transform.center.x,
        0.0,
        "one undo takes back the whole scrub, however long it took"
    );
}
