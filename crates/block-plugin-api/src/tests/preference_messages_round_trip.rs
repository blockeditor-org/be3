use super::*;

#[test]
fn preference_messages_round_trip() {
    for theme in ThemeKind::ALL {
        for motion in Motion::ALL {
            for canvas_touch in CanvasTouch::ALL {
                let message = Message::Preferences(Preferences {
                    theme,
                    motion,
                    canvas_touch,
                });
                assert_eq!(
                    decode_frame(&encode_frame(&message).unwrap()).unwrap(),
                    message
                );
            }
        }
    }
}
