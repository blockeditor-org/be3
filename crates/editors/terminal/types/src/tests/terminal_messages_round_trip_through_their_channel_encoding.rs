use crate::terminal::{Key, Modifiers, TerminalKey, TerminalMessage, TerminalSize, TerminalWheel};
use be_block::presence::ChannelKind;

#[test]
fn terminal_messages_round_trip_through_their_channel_encoding() {
    for message in [
        TerminalMessage::Resize(TerminalSize { cols: 80, rows: 24 }),
        TerminalMessage::Key(TerminalKey {
            key: Key::ArrowUp,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            text: String::new(),
            pressed: true,
        }),
        TerminalMessage::Key(TerminalKey {
            key: Key::Unidentified,
            modifiers: Modifiers::default(),
            text: "é".to_owned(),
            pressed: false,
        }),
        TerminalMessage::Paste("one\ntwo".to_owned()),
        TerminalMessage::Wheel(TerminalWheel {
            lines: -3,
            col: 4,
            row: 7,
        }),
    ] {
        assert_eq!(TerminalMessage::decode(&message.encode()), Some(message));
    }
    assert_eq!(TerminalMessage::decode(&[0xff, 0xff]), None);
}
