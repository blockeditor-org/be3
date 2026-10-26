use super::*;

#[test]
fn every_key_round_trips() {
    assert_eq!(
        Key::ALL.len(),
        Key::ALL.iter().collect::<HashSet<_>>().len()
    );
    for key in Key::ALL {
        let message = InputBatch {
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
        };
        round_trips(&message);
    }
}
