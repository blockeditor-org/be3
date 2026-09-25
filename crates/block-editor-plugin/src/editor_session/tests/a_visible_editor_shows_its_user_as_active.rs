use super::*;

struct BlockIgnoringApp;

impl crate::BeuiApp for BlockIgnoringApp {
    fn view(_editor: crate::Editor) -> beui::NodeId {
        beui::reactive::Frame().build()
    }
}

#[test]
fn a_visible_editor_shows_its_user_as_active() {
    let mut session = EditorSession::new::<BlockIgnoringApp>(EditorInstanceId(0), Waker::default());
    session.connect(Uuid::new_v4(), Uuid::new_v4());

    session.presence_visible(true);
    let shown = session.host.take_shown_presence();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].kind, UserActive::ID);
    assert!(shown[0].value.is_some());

    session.presence_visible(false);
    let hidden = session.host.take_shown_presence();
    assert_eq!(hidden.len(), 1);
    assert!(hidden[0].value.is_none());
}
