use super::*;
use be_block::{BlockContent, CounterContent, LiveEdit};
use block_plugin_api::{HostReply, HostRequest};

#[test]
fn revision_history_editors_cannot_mutate_any_block() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    crate::be::hold(block, CounterContent::CONTENT_TYPE);
    crate::be::operate_from(
        block,
        0,
        CounterContent::encode_operation(&be_block::Counter::add(1)),
    );
    crate::be::flush();
    let head = crate::be::wait_for(std::time::Duration::from_secs(20), |shared| {
        shared.graph.get(block).and_then(|node| node.head)
    })
    .expect("the saved counter has a head");
    let (send, receive) = std::sync::mpsc::channel();
    crate::be::revisions::request(
        block,
        HostRequest::PreviewRevision {
            block_id: block.into_bytes(),
            commit: *head.hash().as_bytes(),
        },
        move |reply| {
            send.send(reply).unwrap();
        },
    );
    let HostReply::RevisionPreviewed(Ok(id)) = receive
        .recv_timeout(std::time::Duration::from_secs(20))
        .unwrap()
    else {
        panic!("preview opens");
    };
    let preview = Uuid::from_bytes(id);
    let mut instances = placed_on(preview, CounterContent::CONTENT_TYPE);
    let before = crate::be::content(block).unwrap().bytes().to_vec();
    for target in [block, preview] {
        assert!(!instances.editor_message(EditorMessage::Operate {
            instance: INSTANCE,
            block_id: target.into_bytes(),
            operation: CounterContent::encode_operation(&be_block::Counter::add(99))
        }));
        assert!(!instances.editor_message(EditorMessage::ReplaceContent {
            instance: INSTANCE,
            block_id: target.into_bytes(),
            content_type: CounterContent::CONTENT_TYPE.into_bytes(),
            bytes: CounterContent::default().encode()
        }));
        assert!(!instances.editor_message(EditorMessage::BlockCommand {
            instance: INSTANCE,
            block_id: target.into_bytes(),
            command: BlockCommand::Undo
        }));
        assert!(!instances.editor_message(EditorMessage::Request {
            instance: INSTANCE,
            request_id: 1,
            request: HostRequest::RestoreRevision {
                block_id: target.into_bytes(),
                commit: *head.hash().as_bytes()
            }
        }));
    }
    let child = Uuid::new_v4();
    crate::be::open(child, CounterContent::CONTENT_TYPE);
    crate::be::operate_from(
        child,
        0,
        CounterContent::encode_operation(&be_block::Counter::add(7)),
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
    assert!(!nested.editor_message(EditorMessage::Operate {
        instance: INSTANCE,
        block_id: child.into_bytes(),
        operation: CounterContent::encode_operation(&be_block::Counter::add(99))
    }));
    crate::be::flush();
    assert_eq!(crate::be::content(block).unwrap().bytes().to_vec(), before);
    crate::be::revisions::release(preview);
    crate::be::revisions::collect_released();
}
