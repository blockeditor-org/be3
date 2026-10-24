use be_block::presence::ChannelKind;
use crate::terminal::{TerminalMessage, TerminalSize};

#[test]
fn terminal_messages_round_trip_through_their_channel_encoding() {
    let size = TerminalSize { cols: 80, rows: 24 };
    for message in [
        TerminalMessage::Attach(size),
        TerminalMessage::Resize(size),
        TerminalMessage::Input(b"ls\r".to_vec()),
        TerminalMessage::Snapshot {
            shell: 2,
            sequence: 9,
            size,
            screen: b"\x1b[2J\x1b[Hhello".to_vec(),
        },
        TerminalMessage::Output {
            shell: 2,
            sequence: 10,
            bytes: vec![0, 255, 27],
        },
        TerminalMessage::Exited {
            shell: 2,
            status: Some(1),
        },
    ] {
        assert_eq!(TerminalMessage::decode(&message.encode()), Some(message));
    }
    assert_eq!(TerminalMessage::decode(&[0xff, 0xff]), None);
}
