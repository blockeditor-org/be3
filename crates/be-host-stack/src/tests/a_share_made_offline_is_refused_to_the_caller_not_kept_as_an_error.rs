use super::*;

/// Sharing needs the server's say, so offline it is not kept for later: the caller hears the
/// refusal, to show where the person shared, and the host keeps no error of it.
#[test]
fn a_share_made_offline_is_refused_to_the_caller_not_kept_as_an_error() {
    let mut harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    written(block, "a note");
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);

    let (answered, answer) = std::sync::mpsc::channel();
    set_access(block, Uuid::new_v4(), be_block_graph::Access::View, move |set| {
        let _ = answered.send(set);
    });
    let refused = answer.recv().expect("the share is answered");
    assert_eq!(
        refused,
        Err("You're offline. Sharing changes need a connection; try again once you're back online."
            .to_owned())
    );
    let (answered, answer) = std::sync::mpsc::channel();
    leave(block, move |left| {
        let _ = answered.send(left);
    });
    assert!(answer.recv().expect("leaving is answered").is_err());
    assert_eq!(status().error, None, "a refusal is not the host's error");
}
