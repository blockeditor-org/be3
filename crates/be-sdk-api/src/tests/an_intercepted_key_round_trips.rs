use super::*;

#[test]
fn an_intercepted_key_round_trips() {
    let message = InputBatch {
        screen: ScreenId(1),
        every_sample: false,
        dismissed: Vec::new(),
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
    };
    round_trips(&message);
}
