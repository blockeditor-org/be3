use super::*;

#[test]
fn an_event_in_an_entry_is_planned_at_its_place_on_the_frame() {
    let frame = PixelRect::new(500, 200, 800, 600);
    let size = vec2(400.0, 300.0);
    let planned = plan(
        DESKTOP,
        frame,
        size,
        &Event::Pointer {
            at: pos2(100.0, 150.0),
            action: PointerAction::Press(PointerButton::Primary),
        },
    );
    let point = PixelPoint::new(700, 500);
    assert_eq!(planned.cursor, Some(point));
    assert_eq!(
        planned.synths,
        pointer(DESKTOP, point, PointerAction::Press(PointerButton::Primary))
    );
    let typed = plan(DESKTOP, frame, size, &Event::Text("hi".to_owned()));
    assert_eq!(typed.cursor, None);
    assert_eq!(typed.synths, text("hi"));
}
