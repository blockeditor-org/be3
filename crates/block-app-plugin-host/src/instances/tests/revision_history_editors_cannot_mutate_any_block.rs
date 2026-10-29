use super::*;
use be_block::{BlockContent, LiveEdit};
use block_plugin_api::{
    embed::host_services::Call,
    ops::{OpError, Outcome, Reply},
};
use counter_types::CounterContent;

#[test]
fn revision_history_editors_cannot_mutate_any_block() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    block_app_stack::hold(block, CounterContent::CONTENT_TYPE);
    block_app_stack::operate(
        block,
        CounterContent::encode_operation(&counter_types::Counter::add(1)),
    );
    block_app_stack::flush();
    let head = block_app_stack::wait_for(std::time::Duration::from_secs(20), |shared| {
        shared.graph.get(block).and_then(|node| node.head)
    })
    .expect("the saved counter has a head");
    let (send, receive) = std::sync::mpsc::channel();
    block_app_stack::revisions::request(
        block,
        block_app_stack::revisions::Asked::Preview(*head.hash().as_bytes()),
        move |reply| {
            send.send(reply).unwrap();
        },
    );
    let block_app_stack::revisions::Answered::Previewed(Ok(preview)) = receive
        .recv_timeout(std::time::Duration::from_secs(20))
        .unwrap()
    else {
        panic!("preview opens");
    };
    let mut instances = placed_on(preview, CounterContent::CONTENT_TYPE);
    let before = block_app_stack::content(block).unwrap().bytes().to_vec();
    for target in [block, preview] {
        assert!(!to_replica(
            &mut instances,
            target,
            &FromEditor::Operate(CounterContent::encode_operation(
                &counter_types::Counter::add(99)
            ))
        ));
        assert!(!call(
            &mut instances,
            wire::Call::ReplaceContent {
                block: target.into_bytes(),
                content_type: CounterContent::CONTENT_TYPE.into_bytes(),
                bytes: CounterContent::default().encode()
            }
        ));
        assert!(!call(
            &mut instances,
            wire::Call::Act {
                block: target.into_bytes(),
                command: BlockCommand::Undo
            }
        ));
        instances.serve(crate::wire::Service {
            instance: INSTANCE,
            op: Some(1),
            call: Call::RestoreRevision {
                reply: Reply::new(1),
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
    block_app_stack::open(child, CounterContent::CONTENT_TYPE);
    block_app_stack::operate(
        child,
        CounterContent::encode_operation(&counter_types::Counter::add(7)),
    );
    block_app_stack::flush();
    block_app_stack::close(child);
    block_app_stack::wait_for(std::time::Duration::from_secs(20), |shared| {
        (!shared.blocks.contains_key(&child)).then_some(())
    })
    .expect("the child's live session closes");
    let descendant = block_app_stack::revisions::alias(preview, child);
    block_app_stack::open(descendant, CounterContent::CONTENT_TYPE);
    block_app_stack::wait_for(std::time::Duration::from_secs(20), |shared| {
        shared
            .revision_previews
            .get(&descendant)
            .and_then(|preview| preview.content.as_ref())
            .filter(|_| shared.graph.get(child).is_some())
            .map(|_| ())
    })
    .expect("the child loads as a read-only preview");
    assert_eq!(block_app_stack::revisions::source(descendant), Some(child));
    assert!(!block_app_stack::access(descendant).can_edit());
    assert_eq!(
        CounterContent::decode(block_app_stack::content(descendant).unwrap().bytes())
            .unwrap()
            .root()
            .value(),
        7
    );
    assert!(
        block_app_stack::content(child).is_none(),
        "a nested preview never joins a live session"
    );
    let mut nested = placed_on(descendant, CounterContent::CONTENT_TYPE);
    assert!(!to_replica(
        &mut nested,
        child,
        &FromEditor::Operate(CounterContent::encode_operation(
            &counter_types::Counter::add(99)
        ))
    ));
    block_app_stack::flush();
    assert_eq!(
        block_app_stack::content(block).unwrap().bytes().to_vec(),
        before
    );
    block_app_stack::revisions::release(preview);
    block_app_stack::revisions::collect_released(|_| {});
}
