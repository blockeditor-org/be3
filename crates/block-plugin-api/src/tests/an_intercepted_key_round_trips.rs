use super::*;

#[test]
fn an_intercepted_key_round_trips() {
    let message = Message::Input(InputBatch {
        screen: ScreenId(1),
        events: vec![
            InputEvent::Modifiers(Modifiers {
                alt: true,
                ..Modifiers::default()
            }),
            InputEvent::InterceptedKey {
                key: Key::Tab,
                pressed: true,
                repeat: false,
            },
            InputEvent::InterceptedTap { key: Key::Logo },
        ],
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
