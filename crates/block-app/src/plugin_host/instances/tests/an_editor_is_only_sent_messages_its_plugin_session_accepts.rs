use super::*;
use be_block::BlockContent;
use counter_types::CounterContent;

use std::time::Duration;

use block_editor_plugin::session::{ClientSession, State};
use block_plugin_api::{HelloAccepted, SurfaceFormat, SurfaceSpec};
use block_type_guest::editor::FromEditor;

#[test]
fn an_editor_is_only_sent_messages_its_plugin_session_accepts() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let block_type = CounterContent::CONTENT_TYPE;
    let mut instances = placed_on(block, block_type);
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    session.receive(Message::HelloAccepted(HelloAccepted {
        host_name: "test host".into(),
        surface: Some(SurfaceSpec {
            format: SurfaceFormat::Rgba8Unorm,
            render: true,
        }),
        preferences: block_plugin_api::Preferences::default(),
    }));

    let opened = next_screens(&mut instances).opened;
    assert!(
        opened
            .iter()
            .any(|message| matches!(message, Message::Editor(EditorMessage::Open { .. })))
    );
    for message in opened {
        session.receive(message);
    }
    instances.editor_message(EditorMessage::Replica {
        instance: INSTANCE,
        block_id: block.into_bytes(),
        frame: block_type_api::encode_frame(&FromEditor::Resend),
    });
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        shared.blocks.contains_key(&block).then_some(())
    })
    .expect("the new stack never held the block's content");
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        shared.editor_frames.contains_key(&INSTANCE.0).then_some(())
    })
    .expect("the replica never answered the editor");

    let carried = next_screens(&mut instances).opened;

    assert!(
        carried
            .iter()
            .any(|message| matches!(message, Message::Editor(EditorMessage::Replica { .. }))),
        "the host never sent the block's content"
    );
    for message in carried {
        session.receive(message);
    }
    assert_eq!(session.state(), State::Running);
}
