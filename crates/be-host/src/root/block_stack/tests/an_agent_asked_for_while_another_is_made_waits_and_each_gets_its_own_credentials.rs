use super::*;

/// The root asks for two agents at once. The host makes one organization request at a time, so
/// the second waits for the first, and starts in the frame that delivers the first: each is
/// answered with its own credentials, and neither is dropped or handed the other's.
#[test]
fn an_agent_asked_for_while_another_is_made_waits_and_each_gets_its_own_credentials() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let mut served = Served::new(true);

    served.step(vec![
        (1, SecretCommand::CreateAgent("First".to_owned())),
        (2, SecretCommand::CreateAgent("Second".to_owned())),
    ]);
    assert_eq!(
        (served.stack.making.len(), served.stack.deferred.len()),
        (1, 1),
        "the first is being made and the second waits for it"
    );

    served.until("both are answered", |served| served.answers.len() == 2);
    assert_eq!(
        served.answers,
        vec![(1, Some("First".to_owned())), (2, Some("Second".to_owned()))],
        "each request is answered with the agent it asked for"
    );
    assert!(served.lists("First") && served.lists("Second"));
}
