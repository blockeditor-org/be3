use super::*;

#[test]
fn a_press_after_typing_waits_for_the_next_frame() {
    let press = Event::PointerButton {
        pos: pos2(10.0, 10.0),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    };
    let mut pending = vec![
        press.clone(),
        Event::Text("a".to_owned()),
        press.clone(),
        Event::Text("b".to_owned()),
    ];

    assert_eq!(next_batch(&mut pending), [press.clone(), Event::Text("a".to_owned())]);
    assert_eq!(next_batch(&mut pending), [press, Event::Text("b".to_owned())]);
    assert!(pending.is_empty());
}
