use super::*;

#[test]
fn every_key_round_trips() {
    assert_eq!(
        Key::ALL.len(),
        Key::ALL.iter().collect::<HashSet<_>>().len()
    );
    for key in Key::ALL {
        let message = Message::Input(InputBatch {
            screen: ScreenId(1),
            every_sample: false,
            events: vec![
                Input::Key(Keystroke {
                    key: Some(key),
                    pressed: true,
                    ..Keystroke::default()
                })
                .into(),
            ],
        });
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
