use super::*;

#[test]
fn an_intercepted_key_round_trips() {
    let message = Message::Input(InputBatch {
        screen: ScreenId(1),
        every_sample: false,
        events: vec![
            Input::Modifiers(Modifiers {
                alt: true,
                ..Modifiers::default()
            })
            .into(),
            Input::InterceptedKey {
                key: Key::Tab,
                pressed: true,
                repeat: false,
            }
            .into(),
            Input::InterceptedTap(Key::Logo).into(),
        ],
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
