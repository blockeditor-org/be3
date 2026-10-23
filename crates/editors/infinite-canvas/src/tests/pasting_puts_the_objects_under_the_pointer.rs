use super::*;
use block_editor_beui::beui::{Input, Pasted};

#[test]
fn pasting_puts_the_objects_under_the_pointer() {
    let mut first = card();
    first.transform.center = CanvasPoint::new(300.0, 300.0);
    let mut second = card();
    second.id = Uuid::from_u128(2);
    second.transform.center = CanvasPoint::new(500.0, 300.0);
    let mut editor = editor(&[card()]);
    let pointer = editor.point_of(&format!("infinite-canvas.entity.{}", card().id));
    let payload = serde_json::json!({ "entities": [first, second] });

    editor.click(pointer);
    editor.run();
    let applied = editor.applied(None);
    editor.step(vec![Input::Paste(Pasted::Text(format!(
        "be3-infinite-canvas:{payload}"
    )))]);
    editor.run();

    assert_eq!(
        editor.applied(None),
        applied + 1,
        "a paste should be one edit, so one undo takes all of it back"
    );
    let pasted = entities(&editor);
    assert_eq!(pasted.len(), 3);
    for (entity, x) in pasted[1..].iter().zip([-100.0, 100.0]) {
        let center = entity.transform.center;
        assert!(
            (center.x - x).abs() < 1.0 && center.y.abs() < 1.0,
            "the paste should be centered on the pointer, over the card at the origin, \
             but put a card at {center:?}"
        );
    }
}
