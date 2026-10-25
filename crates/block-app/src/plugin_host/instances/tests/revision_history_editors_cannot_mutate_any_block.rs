use super::*;
use be_block::{BlockContent, LiveEdit};
use block_plugin_api::{
    embed::host_services::Call,
    ops::{OpError, Outcome, Reply},
};
use counter_types::CounterContent;

#[test]
fn revision_history_editors_cannot_mutate_any_block() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    crate::be::hold(block, CounterContent::CONTENT_TYPE);
    crate::be::operate(
        block,
        CounterContent::encode_operation(&counter_types::Counter::add(1)),
    );
    crate::be::flush();
    let head = crate::be::wait_for(std::time::Duration::from_secs(20), |shared| {
        shared.graph.get(block).and_then(|node| node.head)
    })
    .expect("the saved counter has a head");
    let (send, receive) = std::sync::mpsc::channel();
    crate::be::revisions::request(
        block,
        crate::be::revisions::Asked::Preview(*head.hash().as_bytes()),
        move |reply| {
            send.send(reply).unwrap();
        },
    );
    let crate::be::revisions::Answered::Previewed(Ok(preview)) = receive
        .recv_timeout(std::time::Duration::from_secs(20))
        .unwrap()
    else {
        panic!("preview opens");
    };
    let mut instances = placed_on(preview, CounterContent::CONTENT_TYPE);
    let before = crate::be::content(block).unwrap().bytes().to_vec();
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
        instances.serve(crate::plugin_host::wire::Service {
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
    crate::be::open(child, CounterContent::CONTENT_TYPE);
    crate::be::operate(
        child,
        CounterContent::encode_operation(&counter_types::Counter::add(7)),
    );
    crate::be::flush();
    crate::be::close(child);
    crate::be::wait_for(std::time::Duration::from_secs(20), |shared| {
        (!shared.blocks.contains_key(&child)).then_some(())
    })
    .expect("the child's live session closes");
    let descendant = crate::be::revisions::alias(preview, child);
    crate::be::open(descendant, CounterContent::CONTENT_TYPE);
    crate::be::wait_for(std::time::Duration::from_secs(20), |shared| {
        shared
            .revision_previews
            .get(&descendant)
            .and_then(|preview| preview.content.as_ref())
            .filter(|_| shared.graph.get(child).is_some())
            .map(|_| ())
    })
    .expect("the child loads as a read-only preview");
    assert_eq!(crate::be::revisions::source(descendant), Some(child));
    assert!(!crate::be::access(descendant).can_edit());
    assert_eq!(
        CounterContent::decode(crate::be::content(descendant).unwrap().bytes())
            .unwrap()
            .root()
            .value(),
        7
    );
    assert!(
        crate::be::content(child).is_none(),
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
    crate::be::flush();
    assert_eq!(crate::be::content(block).unwrap().bytes().to_vec(), before);
    crate::be::revisions::release(preview);
    crate::be::revisions::collect_released();
}
