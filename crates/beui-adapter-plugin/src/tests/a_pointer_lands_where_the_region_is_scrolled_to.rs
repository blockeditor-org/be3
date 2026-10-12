use super::*;

#[test]
fn a_pointer_lands_where_the_region_is_scrolled_to() {
    let scrolled = region(
        Rect::from_min_size(pos2(-10.0, -20.0), vec2(100.0, 50.0)),
        [80, 30],
    );
    let input = input::translate(&scrolled, &InputEvent::PointerMoved { x: 5.0, y: 5.0 });
    assert!(
        matches!(input, Some(Input::Event(Event::PointerMoved(at))) if at == pos2(-5.0, -15.0)),
        "{input:?}"
    );
}
