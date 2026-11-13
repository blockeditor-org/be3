use super::*;

use be_sdk_api::VersionCommand;

#[test]
fn a_version_command_waiting_on_a_silent_server_holds_up_no_edit() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let versioned = start_versioning("hello\n");
    show_version(versioned.checkout, be_block::CheckoutContent::CONTENT_TYPE);
    let counter = Uuid::new_v4();
    open(counter, CounterContent::CONTENT_TYPE);
    add(counter, 1);
    wait_for_count(counter, 1);
    flush_and_send();

    harness.relay().stall();
    version(
        versioned.checkout,
        VersionCommand::CreateBranch {
            name: "draft".into(),
        },
    );
    wait_until("started the version command", |shared| {
        version_status(shared, versioned.checkout).is_some_and(|status| status.busy)
    });
    add(counter, 2);
    wait_for_count(counter, 3);
    assert!(
        status().connected,
        "the edit waited for the version command on the silent server"
    );

    harness.relay().cut();
    wait_until("finished the version command once reconnected", |shared| {
        shared.connected && settled(shared, versioned.checkout)
    });
}
