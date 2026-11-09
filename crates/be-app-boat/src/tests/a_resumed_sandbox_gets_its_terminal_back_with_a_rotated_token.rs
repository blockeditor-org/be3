use super::*;

#[test]
fn a_resumed_sandbox_gets_its_terminal_back_with_a_rotated_token() {
    let mut boat = Boat::listing(SANDBOXES);
    let agent = Uuid::new_v4();
    let block = boat.connected(agent);

    let stopped = SANDBOXES.replacen("\"state\": \"idle\"", "\"state\": \"archived\"", 1);
    boat.wait(Duration::from_secs(15));
    let list = boat.fetch();
    boat.answer(&list, 200, &stopped);
    assert!(!boat.test.has_fetches(), "a stopped sandbox is asked nothing");

    boat.wait(Duration::from_secs(15));
    let list = boat.fetch();
    boat.answer(&list, 200, SANDBOXES);
    let check = boat.ran("test -s", MISSING);
    assert!(check.contains(&format!("/run/be3/terminal-host-{block}.json")));

    let (reply, rotated) = boat.host_call("the token rotated", |call| match call {
        HostCall::RotateAgentToken { reply, agent } => Ok((reply, agent)),
        call => Err(call),
    });
    assert_eq!(rotated, agent.into_bytes());
    assert_eq!(
        boat.label(&format!("boat.sandbox.{RUNNING}.terminal.status")),
        "Terminal offline, reconnecting…"
    );
    boat.test
        .snapshot("a_resumed_sandboxs_terminal_is_offline_until_it_reconnects");
    boat.test.answer(reply, Ok("second-token".to_owned()));
    boat.test.run();

    let credentials = boat.install_answered(block, false);
    assert_eq!(credentials["token"], "second-token");
    assert_eq!(
        credentials["recovery_phrase"],
        "abandon ability able about above absent absorb abstract absurd abuse access accident",
        "the whole credentials are written again"
    );
    assert!(boat.kept_terminals()[0].1.credentials.contains("second-token"));
    assert_eq!(
        boat.label(&format!("boat.sandbox.{RUNNING}.terminal.status")),
        "Terminal"
    );

    boat.test.presence_visible(false);
    let mut reopened = boat.reopened();
    let list = reopened.fetch();
    reopened.answer(&list, 200, SANDBOXES);
    reopened.ran("test -s", PRESENT);
    assert!(
        reopened.kept_terminals()[0]
            .1
            .credentials
            .contains("second-token"),
        "the credentials are kept across a reload"
    );
    assert_eq!(
        reopened.label(&format!("boat.sandbox.{RUNNING}.terminal.status")),
        "Terminal"
    );
    reopened.test.settle();
    assert!(
        reopened
            .test
            .take_requests_where(|call| match call {
                HostCall::RotateAgentToken { .. } => Ok(()),
                call => Err(call),
            })
            .is_empty(),
        "a sandbox that still has its credentials keeps its token"
    );
}
