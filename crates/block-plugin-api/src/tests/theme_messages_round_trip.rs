use super::*;

#[test]
fn theme_messages_round_trip() {
    for message in [
        Message::Theme(Theme::Dark),
        Message::Motion(Motion::Animated),
        Message::Motion(Motion::Instant),
        Message::Theme(Theme::OledDark),
        Message::Theme(Theme::Light),
        Message::Theme(Theme::EInk),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
