use super::*;

/// A host that read what the account owns before its roots were listed, and has not yet heard
/// that the graph changed when the stack already answers that the roots are ready (an offline
/// start lists them from what the device keeps, between two frames), finds the settings the
/// account has rather than making a second one beside them.
#[test]
fn settings_are_not_made_again_by_a_host_that_has_not_heard_the_roots_listed() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    be_host_stack::wait_until(|shared| {
        shared
            .graph
            .ready(be_host_stack::Query::Roots)
            .then_some(())
    });
    let made = RootSettings::new(WORKSPACE_EDITOR)
        .ensure()
        .expect("the settings are made");

    let mut restarted = RootSettings::new(WORKSPACE_EDITOR);
    restarted.cache.borrow_mut().owned = Some(Vec::new());
    assert_eq!(restarted.ensure(), Some(made));
    assert_eq!(restarted.created, None, "it made no settings of its own");
    let settings: Vec<Uuid> = be_host_stack::query(be_host_stack::Query::Roots)
        .into_iter()
        .filter(|node| node.content_type == Settings::CONTENT_TYPE)
        .map(|node| node.id)
        .collect();
    assert_eq!(settings, vec![made], "the account has one settings");
}
