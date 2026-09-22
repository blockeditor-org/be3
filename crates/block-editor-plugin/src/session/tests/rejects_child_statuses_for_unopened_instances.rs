use super::*;
use block_plugin_api::{ChildId, ChildStatus, EditorCapabilities, InteractionMode, ResizeMode};

fn status(instance: EditorInstanceId) -> ChildStatus {
    ChildStatus {
        instance,
        region: EditorRegion::Frame,
        child: ChildId(1),
        available: true,
        intrinsic: Some(Size {
            width: 320.0,
            height: 180.0,
        }),
        aspect_ratio: None,
        hovered: false,
        active: false,
        interaction: InteractionMode::Live,
        capabilities: EditorCapabilities::default(),
        resize: ResizeMode::Both,
        error: None,
    }
}

#[test]
fn rejects_child_statuses_for_unopened_instances() {
    let mut session = ClientSession::default();
    accept(&mut session);
    let instance = EditorInstanceId(1);
    open(&mut session, instance);
    assert!(
        session
            .receive(Message::ChildStatuses(vec![status(instance)]))
            .is_empty()
    );
    assert_eq!(session.state(), State::Running);

    let responses = session.receive(Message::ChildStatuses(vec![status(EditorInstanceId(9))]));
    assert!(matches!(responses.as_slice(), [Message::Error(_)]));
    assert_eq!(session.state(), State::Failed);
}
