use super::*;
use be_block::{BlockContent, LiveEdit};
use be_sdk_api::BlockCommand;
use be_sdk_api::{
    embed::host_services::Call,
    ops::{OpError, Outcome},
};
use be_type_example_counter::CounterContent;

#[test]
fn revision_history_editors_cannot_mutate_any_block() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    be_host_stack::hold(block, CounterContent::CONTENT_TYPE);
    be_host_stack::operate(
        block,
        CounterContent::encode_operation(&be_type_example_counter::Counter::add(1)),
    );
    be_host_stack::flush_and_send();
    let head =
        be_host_stack::wait_until(|shared| shared.graph.get(block).and_then(|node| node.head));
    let (send, receive) = std::sync::mpsc::channel();
    be_host_stack::revisions::request(
        block,
        be_host_stack::revisions::Asked::Preview(*head.hash().as_bytes()),
        move |reply| {
            send.send(reply).unwrap();
        },
    );
    let be_host_stack::revisions::Answered::Previewed(Ok(preview)) = receive.recv().unwrap() else {
        panic!("preview opens");
    };
    let mut instances = placed_on(preview, CounterContent::CONTENT_TYPE);
    let before = be_host_stack::content(block).unwrap().bytes().to_vec();
    for target in [block, preview] {
        assert!(!to_replica(
            &mut instances,
            target,
            &FromEditor::Operate(CounterContent::encode_operation(
                &be_type_example_counter::Counter::add(99)
            ))
        ));
        assert!(!call(
            &mut instances,
            wire::Call::WriteContent {
                reply: reply(next_op()),
                block: target.into_bytes(),
                content_type: CounterContent::CONTENT_TYPE.into_bytes(),
                bytes: CounterContent::default().encode(),
                replace: true,
            }
        ));
        assert!(!call(
            &mut instances,
            wire::Call::Act {
                reply: reply(next_op()),
                block: target.into_bytes(),
                command: BlockCommand::Undo
            }
        ));
        instances.take_answers();
        instances.serve(crate::wire::Service {
            instance: INSTANCE,
            call: Call::RestoreRevision {
                reply: reply(1),
                block: target.into_bytes(),
                commit: *head.hash().as_bytes(),
            },
        });
        assert_eq!(
            instances.take_answers(),
            vec![(1, Outcome::Err(OpError::Denied))]
        );
    }
    let child = Uuid::new_v4();
    be_host_stack::open(child, CounterContent::CONTENT_TYPE);
    be_host_stack::operate(
        child,
        CounterContent::encode_operation(&be_type_example_counter::Counter::add(7)),
    );
    be_host_stack::flush_and_send();
    be_host_stack::close(child);
    be_host_stack::wait_until(|shared| (!shared.blocks.contains_key(&child)).then_some(()));
    let descendant = be_host_stack::revisions::alias(preview, child);
    be_host_stack::open(descendant, CounterContent::CONTENT_TYPE);
    be_host_stack::wait_until(|shared| {
        shared
            .revision_previews
            .get(&descendant)
            .and_then(|preview| preview.content.as_ref())
            .filter(|_| shared.graph.get(child).is_some())
            .map(|_| ())
    });
    assert_eq!(be_host_stack::revisions::source(descendant), Some(child));
    assert!(!be_host_stack::access(descendant).can_edit());
    assert_eq!(
        CounterContent::decode(be_host_stack::content(descendant).unwrap().bytes())
            .unwrap()
            .root()
            .value(),
        7
    );
    assert!(
        be_host_stack::content(child).is_none(),
        "a nested preview never joins a live session"
    );
    let mut nested = placed_on(descendant, CounterContent::CONTENT_TYPE);
    assert!(!to_replica(
        &mut nested,
        child,
        &FromEditor::Operate(CounterContent::encode_operation(
            &be_type_example_counter::Counter::add(99)
        ))
    ));
    be_host_stack::flush_and_send();
    assert_eq!(
        be_host_stack::content(block).unwrap().bytes().to_vec(),
        before
    );
    be_host_stack::revisions::release(preview);
    be_host_stack::revisions::collect_released(|_| {});
}
