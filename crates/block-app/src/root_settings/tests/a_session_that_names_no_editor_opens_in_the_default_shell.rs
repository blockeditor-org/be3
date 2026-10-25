use super::*;

#[test]
fn a_session_that_names_no_editor_opens_in_the_default_shell() {
    let harness = be::Harness::start();
    harness.connect();
    let session = Uuid::new_v4();
    be::create(
        session,
        VIEW_TYPE,
        BlockParent::Root,
        BlockMetadata::default(),
        None,
    );
    be::hold(session, VIEW_TYPE);
    be::wait_for(Duration::from_secs(20), |shared| {
        shared.blocks.contains_key(&session).then_some(())
    })
    .expect("the new stack never held the session");

    let settings = RootSettings::new(WORKSPACE_EDITOR);
    assert_eq!(settings.session_editor(session), Some(WORKSPACE_EDITOR));
}
