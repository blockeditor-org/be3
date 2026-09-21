use super::*;

use std::time::{Duration, Instant};

use block::Block;
use block_editor_plugin::session::{ClientSession, State};
use block_plugin_api::{Capability, HelloAccepted, PROTOCOL_VERSION};

#[test]
fn a_migrated_editor_is_only_sent_messages_its_plugin_session_accepts() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let block_type = block_client::blocks::counter::Counter::TYPE_ID;
    let (mut instances, ..) = placed_on(block, block_type);
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    session.receive(Message::HelloAccepted(HelloAccepted {
        version: PROTOCOL_VERSION,
        host_name: "test host".into(),
        capabilities: vec![Capability::Lifecycle, Capability::Input],
        dark_theme: true,
    }));

    let mut content = false;
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline && !content {
        for message in instances.next_screens(PASS).opened {
            content |= matches!(message, Message::Editor(EditorMessage::Content { .. }));
            session.receive(message);
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(content, "the host never sent the block's content");
    assert_eq!(session.state(), State::Running);
}
