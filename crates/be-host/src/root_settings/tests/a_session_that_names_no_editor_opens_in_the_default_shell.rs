use super::*;

#[test]
fn a_session_that_names_no_editor_opens_in_the_default_shell() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let session = Uuid::new_v4();
    be_host_stack::create(
        session,
        VIEW_TYPE,
        BlockParent::Root,
        BlockMetadata::default(),
        None,
    );
    be_host_stack::hold(session, VIEW_TYPE);
    be_host_stack::wait_until(|shared| shared.blocks.contains_key(&session).then_some(()));

    let settings = RootSettings::new(WORKSPACE_EDITOR);
    assert_eq!(settings.session_editor(session), Some(WORKSPACE_EDITOR));
}
