use super::*;

#[test]
fn a_session_that_names_no_editor_opens_in_the_default_shell() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let session = Uuid::new_v4();
    block_app_stack::create(
        session,
        VIEW_TYPE,
        BlockParent::Root,
        BlockMetadata::default(),
        None,
    );
    block_app_stack::hold(session, VIEW_TYPE);
    block_app_stack::wait_for(Duration::from_secs(20), |shared| {
        shared.blocks.contains_key(&session).then_some(())
    })
    .expect("the new stack never held the session");

    let settings = RootSettings::new(WORKSPACE_EDITOR);
    assert_eq!(settings.session_editor(session), Some(WORKSPACE_EDITOR));
}
