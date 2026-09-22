use super::*;

#[test]
fn theme_messages_round_trip() {
    for message in [
        Message::Theme(Theme { dark: true }),
        Message::Theme(Theme { dark: false }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
