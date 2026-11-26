use super::*;

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_api::{
    BlockMember, Failure,
    blocks::{BlockSubject, BlockValue},
};
use be_type_example_counter::{Counter, CounterContent};

/// The members the watch is answered with next, waiting for the server's names to come back.
fn members(instances: &mut Instances, watch: OpId) -> Result<Vec<BlockMember>, Failure> {
    loop {
        let seen = be_host_base::host::letters_posted();
        next_screens(instances);
        instances.pending();
        if let Some(value) = answered::<BlockValue>(instances, watch) {
            match value {
                BlockValue::Members(members) => return members,
                other => panic!("not members: {other:?}"),
            }
        }
        be_host_stack::take_updates_or_wait("the members the server named", || {
            be_host_base::host::letters_posted() > seen
        });
    }
}

/// Waits until the editor's members are `accounts`, sorted.
fn members_are(instances: &mut Instances, watch: OpId, accounts: &[Uuid]) {
    loop {
        let mut named: Vec<Uuid> = members(instances, watch)
            .expect("the server named the members")
            .into_iter()
            .map(|member| Uuid::from_bytes(member.account))
            .collect();
        named.sort();
        if named == accounts {
            return;
        }
    }
}

/// When the publish follows the revoke.
#[derive(Clone, Copy, Debug)]
enum After {
    /// At once, the revoke still on its way.
    Sent,
    /// Once the mirror heard it, the reseal under the new key perhaps still under way.
    Heard,
    /// Once the mirror heard the open block resealed under the new key.
    Resealed,
}

#[test]
fn a_publish_after_a_revoke_lands_on_an_open_block() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let me = be_host_stack::account().expect("signed in");
    let (block, _) = saved_counter();
    pollster::block_on(be_host_stack::create_agent("Helper".to_owned()))
        .expect("the agent is made");
    let agent = be_host_stack::wait_until("the agent is a contact", |shared| {
        shared
            .contacts
            .iter()
            .find(|contact| contact.display_name == "Helper" && contact.public.is_some())
            .map(|contact| contact.account)
    });
    let mut instances = placed_on(block, CounterContent::CONTENT_TYPE);
    let watch = open_watch(&mut instances, |op| Call::BlockWatchNew {
        resource: wire::BlockWatch(op),
        subject: BlockSubject::Members,
    });
    members_are(&mut instances, watch, &[me]);
    let mut both = vec![me, agent];
    both.sort();

    for after in [After::Sent, After::Heard, After::Resealed] {
        be_host_stack::set_access(block, agent, be_block_graph::Access::Edit, |_| {});
        members_are(&mut instances, watch, &both);
        let (granted, head) = be_host_stack::node(block)
            .map(|node| (node.changed, node.head))
            .expect("the block is in the mirror");

        // The revoke reseals the open block under a new key; a publish after it, wherever it
        // falls in that, lands with the flush that sends it.
        be_host_stack::set_access(block, agent, be_block_graph::Access::None, |_| {});
        if let After::Heard | After::Resealed = after {
            let revoked = be_host_stack::wait_until("the mirror heard the revoke", |shared| {
                shared
                    .graph
                    .get(block)
                    .filter(|node| node.changed > granted)
                    .map(|node| node.changed)
            });
            if let After::Resealed = after {
                be_host_stack::wait_until("the mirror heard the reseal", |shared| {
                    shared
                        .graph
                        .get(block)
                        .is_some_and(|node| node.changed > revoked)
                        .then_some(())
                });
            }
        }
        be_host_stack::operate(block, CounterContent::encode_operation(&Counter::add(1)));
        be_host_stack::flush_and_send();
        be_host_stack::wait_until("the mirror heard the publish", |shared| {
            shared
                .graph
                .get(block)
                .is_some_and(|node| node.head.is_some() && node.head != head)
                .then_some(())
        });
        members_are(&mut instances, watch, &[me]);
    }
}
