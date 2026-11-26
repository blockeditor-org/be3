use super::*;
use be_sdk_beui::beui::Vec2;

#[test]
fn dragging_the_opacity_slider_previews_and_is_written_once_released() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(120.0, 80.0), 0.0);
    rectangle.style.fill = CanvasFill::Solid;
    let mut editor = vector_editor(std::slice::from_ref(&rectangle));
    editor.click(format!("infinite-canvas.entity.{}", rectangle.id));
    editor.reveal("infinite-canvas.opacity");
    let before = entities(&editor)[0].style.opacity;
    editor.store().take_written(None);

    let slider = editor.rect_of("infinite-canvas.opacity");
    let from = slider.center() + Vec2::new(slider.width() / 2.0 - 2.0, 0.0);
    editor.down(from, PointerButton::Primary);
    editor.hover(from - Vec2::new(slider.width() / 4.0, 0.0));
    editor.hover(slider.center());
    let previewed = entities(&editor)[0].style.opacity;
    assert!(
        previewed < before,
        "the drag previews in the block: {previewed} from {before}"
    );
    assert!(
        editor.store().take_written(None).is_empty(),
        "nothing of a drag not yet released is written"
    );

    editor.up(slider.center(), PointerButton::Primary);
    assert_eq!(entities(&editor)[0].style.opacity, previewed);
    assert!(!editor.store().take_written(None).is_empty());
    undo(&mut editor);
    assert_eq!(
        entities(&editor)[0].style.opacity,
        before,
        "one undo takes back the whole drag"
    );
}
