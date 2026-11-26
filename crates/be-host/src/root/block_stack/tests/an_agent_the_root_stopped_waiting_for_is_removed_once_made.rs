use super::*;

/// An agent's credentials cannot be had again, so one made for a root that no longer waits for
/// it is removed from the organization, as `be3:root`'s `make-secret` says.
#[test]
fn an_agent_the_root_stopped_waiting_for_is_removed_once_made() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let mut served = Served::new(false);

    served.step(vec![(1, SecretCommand::CreateAgent("Unwanted".to_owned()))]);
    served.until("the agent is made", |served| !served.answers.is_empty());
    assert_eq!(served.answers, vec![(1, Some("Unwanted".to_owned()))]);
    assert!(
        served.lists("Unwanted"),
        "the organization lists the agent just made"
    );
    assert!(
        served.core.pending_organization_request.is_some(),
        "the host asked to remove it as the root was answered"
    );

    served.until("the removal is answered", |served| {
        served.core.pending_organization_request.is_none()
    });
    assert!(
        !served.lists("Unwanted"),
        "the organization no longer lists the agent nobody waited for"
    );
    assert_eq!(served.answers.len(), 1, "the root is answered once");
}
