use super::*;

use be_sdk_api::{
    BlockMember, Failure,
    blocks::{BlockSubject, BlockValue},
};
use be_type_example_counter::CounterContent;

/// What the watch is answered next, waiting for the server's names to come back.
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
        // The stack's reply reaches the runtime's mailbox as its updates are taken.
        be_host_stack::take_updates_or_wait("the members the server named", || {
            be_host_base::host::letters_posted() > seen
        });
    }
}

#[test]
fn an_editor_watching_its_blocks_members_is_told_their_names() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    be_host_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    be_host_stack::flush_and_send();
    let mut instances = placed_on(block, CounterContent::CONTENT_TYPE);
    let watch = open_watch(&mut instances, |op| Call::BlockWatchNew {
        resource: wire::BlockWatch(op),
        subject: BlockSubject::Members,
    });

    // The block's only member is its author, this account, named by the name it gave the
    // server, which is its own.
    let named = members(&mut instances, watch).expect("the server named the members");
    let me = be_host_stack::account().expect("signed in");
    assert_eq!(named.len(), 1, "{named:?}");
    assert_eq!(named[0].account, me.into_bytes());
    assert!(named[0].verified);
    assert!(!named[0].name.is_empty());
}
