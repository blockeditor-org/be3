use super::*;

use std::time::Duration;

use block::Block;
use block_editor_plugin::session::{ClientSession, State};
use block_plugin_api::{
    DEFAULT_SURFACE_SIDE, HelloAccepted, PROTOCOL_VERSION, SurfaceFormat, SurfaceSpec, Theme,
};

#[test]
fn a_migrated_editor_is_only_sent_messages_its_plugin_session_accepts() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let block_type = block_client::blocks::counter::Counter::TYPE_ID;
    let mut instances = placed_on(block, block_type);
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    session.receive(Message::HelloAccepted(HelloAccepted {
        version: PROTOCOL_VERSION,
        host_name: "test host".into(),
        surface: Some(SurfaceSpec {
            format: SurfaceFormat::Rgba8Unorm,
            max_side: DEFAULT_SURFACE_SIDE,
        }),
        theme: Theme { dark: true },
    }));

    let opened = instances.next_screens(PASS).opened;
    assert!(
        opened
            .iter()
            .any(|message| matches!(message, Message::Editor(EditorMessage::Open { .. })))
    );
    for message in opened {
        session.receive(message);
    }
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        shared.blocks.contains_key(&block).then_some(())
    })
    .expect("the new stack never held the block's content");

    let carried = instances.next_screens(PASS).opened;

    assert!(
        carried
            .iter()
            .any(|message| matches!(message, Message::Editor(EditorMessage::Content { .. }))),
        "the host never sent the block's content"
    );
    for message in carried {
        session.receive(message);
    }
    assert_eq!(session.state(), State::Running);
}
