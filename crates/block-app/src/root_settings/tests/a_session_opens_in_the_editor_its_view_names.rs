use super::*;

#[test]
fn a_session_opens_in_the_editor_its_view_names() {
    let harness = be::Harness::start();
    harness.connect();
    let session = Uuid::new_v4();
    be::create(
        session,
        EditorView::CONTENT_TYPE,
        BlockParent::Root,
        EditorView::metadata(LINUX_DESKTOP_EDITOR, None),
        Some(EditorView::document(WORKSPACE_EDITOR, None).encode()),
    );
    be::hold(session, EditorView::CONTENT_TYPE);
    be::wait_for(Duration::from_secs(20), |shared| {
        shared.blocks.contains_key(&session).then_some(())
    })
    .expect("the new stack never held the session");

    let settings = RootSettings::new(WORKSPACE_EDITOR);
    assert_eq!(
        settings.session_editor(session),
        Some(LINUX_DESKTOP_EDITOR),
        "the host reads the editor from the view's metadata, never its content"
    );
}
