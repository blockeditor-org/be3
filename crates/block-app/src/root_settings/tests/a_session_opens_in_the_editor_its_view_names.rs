use super::*;

#[test]
fn a_session_opens_in_the_editor_its_view_names() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let session = Uuid::new_v4();
    block_app_stack::create(
        session,
        VIEW_TYPE,
        BlockParent::Root,
        BlockMetadata::view(DESKTOP_EDITOR, None),
        None,
    );
    block_app_stack::hold(session, VIEW_TYPE);
    block_app_stack::wait_until(|shared| shared.blocks.contains_key(&session).then_some(()));

    let settings = RootSettings::new(WORKSPACE_EDITOR);
    assert_eq!(settings.session_editor(session), Some(DESKTOP_EDITOR));
}
