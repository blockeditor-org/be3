use super::*;

#[test]
fn theme_messages_round_trip() {
    for message in [
        Message::Theme(Theme {
            dark: true,
            ..Theme::default()
        }),
        Message::Theme(Theme {
            dark: false,
            ..Theme::default()
        }),
        Message::Theme(Theme {
            dark: true,
            motion: Motion::Still,
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
