use super::*;

#[test]
fn removing_an_agent_the_server_no_longer_has_succeeds() {
    let harness = Harness::start();
    harness.connect();
    let (removed, answer) = std::sync::mpsc::channel();
    remove_block_agent(Uuid::new_v4(), move |result| {
        let _ = removed.send(result);
    });
    assert_eq!(
        answer.recv_timeout(Duration::from_secs(30)),
        Ok(Ok(())),
        "an agent already gone is as removed as asked"
    );
}
