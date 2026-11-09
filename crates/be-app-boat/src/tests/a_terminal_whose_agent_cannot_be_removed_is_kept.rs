use super::*;

#[test]
fn a_terminal_whose_agent_cannot_be_removed_is_kept() {
    let mut boat = Boat::listing(SANDBOXES);
    let agent = Uuid::new_v4();
    let block = boat.connected(agent);

    boat.test
        .click(&format!("boat.sandbox.{RUNNING}.terminal.disconnect"));
    boat.test.run();
    let (reply, _) = boat.host_call("the agent removed", |call| match call {
        HostCall::RemoveAgent { reply, agent } => Ok((reply, agent)),
        call => Err(call),
    });
    boat.ran("sudo systemctl disable --now", DONE);
    boat.test
        .answer(reply, Err(Failure::Failed("be3 can't be reached".to_owned())));
    boat.test.run();
    boat.test.settle();

    assert_eq!(
        boat.kept_terminals().len(),
        1,
        "the agent still lives, so its credentials are kept"
    );
    assert!(
        boat.label(&format!("boat.sandbox.{RUNNING}.terminal.status"))
            .contains("could not be removed")
    );
    assert!(
        boat.test
            .shown(&format!("boat.sandbox.{RUNNING}.terminal.disconnect")),
        "it can be disconnected again"
    );
    assert!(
        boat.test
            .content::<BoatContent>(Some(boat.block))
            .root()
            .terminals_of(RUNNING)
            .contains(&block)
    );
}
