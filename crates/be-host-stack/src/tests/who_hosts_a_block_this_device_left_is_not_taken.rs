use super::*;

/// Who hosts a block counts only while this device's replica of it is in its session: told of a
/// block it left (the answer to a join the replica left before it came, such as the rejoin that
/// dropped a claim as its editor closed), the device keeps no word of it, so a component does
/// not start there on that word as the block opens again (plans/hosted_blocks.md, step 4d2).
#[test]
fn who_hosts_a_block_this_device_left_is_not_taken() {
    let harness = Harness::start();
    harness.connect_personal();
    let [left, open_one] = [Uuid::new_v4(), Uuid::new_v4()];
    for block in [left, open_one] {
        create(
            block,
            CounterContent::CONTENT_TYPE,
            be_block_graph::BlockParent::Root,
            be_block_content::BlockMetadata::named("World"),
            None,
            false,
        );
    }
    flush_and_send();
    for block in [left, open_one] {
        open(block, CounterContent::CONTENT_TYPE);
    }
    wait_until("joined both sessions", |shared| {
        shared.session_hosts.contains_key(&left) && shared.session_hosts.contains_key(&open_one)
    });
    let another = be_block_protocol::SessionHost {
        client: 99,
        account: Uuid::new_v4(),
    };
    hear_host(left, Some(another), false);
    wait_until("heard another device hosts the block while it is open", |shared| {
        shared.session_hosts.get(&left) == Some(&Some(false))
    });

    close(left);
    wait_until("left its session", |shared| {
        !shared.session_hosts.contains_key(&left)
    });
    // That no one hosts it, heard once it left; the open block's word after it is the sign the
    // worker took both, as it takes them in order.
    hear_host(left, None, true);
    hear_host(open_one, Some(another), false);
    wait_until("heard of the other block", |shared| {
        shared.session_hosts.get(&open_one) == Some(&Some(false))
    });
    assert!(
        !may_host(left),
        "a block this device left keeps no word of who hosts it"
    );
    assert_eq!(session_host(left), None);
}
