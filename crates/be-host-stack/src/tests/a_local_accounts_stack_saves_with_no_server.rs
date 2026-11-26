use super::*;

/// A local account's stack runs with no server (guides/blocks/client.md, Saving and the outbox): an edit is saved as
/// it is made, nothing waits to be sent, it is never offline, and what it made is there when
/// it starts again.
#[test]
fn a_local_accounts_stack_saves_with_no_server() {
    let harness = Harness::start();
    let organization = Uuid::now_v7();
    let local = || Config {
        local: true,
        server_url: String::new(),
        organization,
        ..harness.config(harness.directory.join("local"))
    };
    start(local());
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 2);
    wait_for_count(block, 2);
    flush();
    wait_until("saved what was made", |shared| {
        shared.local && shared.reached && shared.waiting == 0 && !shared.draining
    });
    let saved = status();
    assert!(saved.local && !saved.connecting);
    assert_eq!(saved.waiting, 0, "nothing waits for a server");
    stop();

    start(local());
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 2);
    add(block, 3);
    wait_for_count(block, 5);
    flush();
    wait_until("saved the edit after a restart", |shared| {
        shared.waiting == 0 && !shared.draining
    });
    assert_eq!(status().waiting, 0);
}
