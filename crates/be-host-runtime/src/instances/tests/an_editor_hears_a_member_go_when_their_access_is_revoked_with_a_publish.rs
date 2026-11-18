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

fn accounts(members: Vec<BlockMember>) -> Vec<Uuid> {
    let mut accounts: Vec<Uuid> = members
        .into_iter()
        .map(|member| Uuid::from_bytes(member.account))
        .collect();
    accounts.sort();
    accounts
}

#[test]
fn an_editor_hears_a_member_go_when_their_access_is_revoked_with_a_publish() {
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
    let made = be_host_stack::wait_until("the block is made", |shared| {
        shared.graph.get(block).map(|node| node.changed)
    });
    be_host_stack::set_access(block, agent, be_block_graph::Access::Edit, |_| {});
    be_host_stack::wait_until("the mirror heard the grant", |shared| {
        shared
            .graph
            .get(block)
            .is_some_and(|node| node.changed > made)
            .then_some(())
    });

    let mut instances = placed_on(block, CounterContent::CONTENT_TYPE);
    let watch = open_watch(&mut instances, |op| Call::BlockWatchNew {
        resource: wire::BlockWatch(op),
        subject: BlockSubject::Members,
    });
    let mut both = vec![me, agent];
    both.sort();
    assert_eq!(
        accounts(members(&mut instances, watch).expect("the server named the members")),
        both
    );

    // A publish and then a revoke, which reseals the open block under a new key, all landing
    // before the editor's next frame: the block's head moves with its change every time the
    // editor looks, and the members are asked for again all the same.
    let (granted, head) = be_host_stack::node(block)
        .map(|node| (node.changed, node.head))
        .expect("the block is in the mirror");
    be_host_stack::operate(block, CounterContent::encode_operation(&Counter::add(1)));
    be_host_stack::flush_and_send();
    // The mirror takes the new head as it publishes, and the change once the server tells it.
    let published = be_host_stack::wait_until("the mirror heard the publish", |shared| {
        shared
            .graph
            .get(block)
            .filter(|node| node.head.is_some() && node.head != head && node.changed > granted)
            .map(|node| node.changed)
    });
    be_host_stack::set_access(block, agent, be_block_graph::Access::None, |_| {});
    let revoked = be_host_stack::wait_until("the mirror heard the revoke", |shared| {
        shared
            .graph
            .get(block)
            .filter(|node| node.changed > published)
            .map(|node| node.changed)
    });
    // The block is resealed under its new key once the revoke ends.
    let resealed = be_host_stack::wait_until("the mirror heard the reseal", |shared| {
        shared
            .graph
            .get(block)
            .filter(|node| node.changed > revoked)
            .map(|node| node.changed)
    });
    let taken = || be_host_stack::node(block).is_some_and(|node| node.changed >= resealed);
    while !taken() {
        be_host_stack::take_updates_or_wait("the reseal", taken);
    }

    assert_eq!(
        accounts(members(&mut instances, watch).expect("the server named the members")),
        [me]
    );
}

