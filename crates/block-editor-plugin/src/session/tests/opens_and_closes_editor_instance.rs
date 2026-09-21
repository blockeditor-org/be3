use super::*;

#[test]
fn opens_and_closes_editor_instance() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    accept(&mut session);
    let instance = EditorInstanceId(4);
    let opened = session.receive(Message::Editor(block_plugin_api::EditorMessage::Open {
        instance,
        block_id: [1; 16],
        block_type: [2; 16],
        account_id: [3; 16],
        workspace_id: [4; 16],
        client_id: [5; 16],
        editable: true,
    }));
    assert_eq!(opened, Vec::new());
    let resized = session.receive(Message::Editor(block_plugin_api::EditorMessage::Resized {
        instance,
        width: 10.0,
        height: 10.0,
    }));
    assert_eq!(resized, Vec::new());
    assert_eq!(session.state(), State::Running);
    let closed = session.receive(Message::Editor(block_plugin_api::EditorMessage::Close {
        instance,
    }));
    assert_eq!(closed, Vec::new());
    assert_eq!(session.state(), State::Running);
    let stray = session.receive(Message::Editor(block_plugin_api::EditorMessage::Resized {
        instance,
        width: 10.0,
        height: 10.0,
    }));
    assert!(matches!(stray.as_slice(), [Message::Error(_)]));
    assert_eq!(session.state(), State::Failed);
}
