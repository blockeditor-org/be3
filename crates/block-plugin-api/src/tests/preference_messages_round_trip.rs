use super::*;

#[test]
fn preference_messages_round_trip() {
    for theme in ThemeKind::ALL {
        for motion in Motion::ALL {
            let message = Message::Preferences(Preferences { theme, motion });
            assert_eq!(
                decode_frame(&encode_frame(&message).unwrap()).unwrap(),
                message
            );
        }
    }
}
