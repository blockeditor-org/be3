use super::*;

#[test]
fn a_terminal_of_a_sandbox_no_longer_listed_can_be_removed() {
    let mut boat = Boat::listing(SANDBOXES);
    let agent = Uuid::new_v4();
    let block = boat.connected(agent);

    boat.wait(Duration::from_secs(15));
    let list = boat.fetch();
    boat.answer(&list, 200, EMPTY);
    assert!(boat.test.shown("boat.gone"));

    boat.test.click(&format!("boat.gone.{block}.remove"));
    boat.test.run();
    let (reply, removed) = boat.host_call("the agent removed", |call| match call {
        HostCall::RemoveAgent { reply, agent } => Ok((reply, agent)),
        call => Err(call),
    });
    assert_eq!(removed, agent.into_bytes());
    boat.test.answer(reply, Ok(()));
    boat.test.run();
    assert!(
        !boat.test.has_fetches(),
        "a sandbox not listed is asked nothing"
    );
    assert!(boat.kept_terminals().is_empty());
    assert!(!boat.test.shown("boat.gone"));
}
