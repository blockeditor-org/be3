use super::*;
use block_plugin_api::{HostReply, HostRequest};

fn request(block: Uuid, request: HostRequest) -> HostReply {
    let (send, receive) = std::sync::mpsc::channel();
    revisions::request(block, request, move |reply| {
        send.send(reply).unwrap();
    });
    receive
        .recv_timeout(PATIENCE)
        .expect("revision request answers")
}

#[test]
fn revision_history_previews_and_restores_saved_content() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    add(block, 5);
    wait_for_count(block, 5);
    flush();
    let first = node(block).unwrap().head.unwrap();
    add(block, 2);
    wait_for_count(block, 7);
    flush();
    let second = node(block).unwrap().head.unwrap();
    assert_ne!(first, second);
    let HostReply::RevisionsListed(Ok(entries)) =
        request(block, HostRequest::ListRevisions(block.into_bytes()))
    else {
        panic!("history is listed");
    };
    assert!(
        entries
            .iter()
            .any(|entry| entry.commit == *first.hash().as_bytes())
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.commit == *second.hash().as_bytes() && entry.current)
    );
    assert!(entries.windows(2).all(|pair| pair[0].time >= pair[1].time));

    let HostReply::RevisionPreviewed(Ok(preview)) = request(
        block,
        HostRequest::PreviewRevision {
            block_id: block.into_bytes(),
            commit: *first.hash().as_bytes(),
        },
    ) else {
        panic!("the revision previews");
    };
    let preview = Uuid::from_bytes(preview);
    let old = content(preview).expect("preview content exists");
    assert_eq!(
        CounterContent::decode(&old.bytes).unwrap().root().value(),
        5
    );
    assert!(!access(preview).can_edit());
    add(block, 3);
    wait_for_count(block, 10);
    assert_eq!(
        content(preview).unwrap().bytes,
        old.bytes,
        "live edits do not reach the preview"
    );
    assert!(matches!(
        request(
            block,
            HostRequest::RestoreRevision {
                block_id: block.into_bytes(),
                commit: *first.hash().as_bytes()
            }
        ),
        HostReply::RevisionRestored(Ok(()))
    ));
    wait_for_count(block, 5);
    let restored = node(block).unwrap().head.unwrap();
    assert_ne!(restored, first);
    let HostReply::RevisionsListed(Ok(entries)) =
        request(block, HostRequest::ListRevisions(block.into_bytes()))
    else {
        panic!("history is listed again");
    };
    assert!(
        entries
            .iter()
            .any(|entry| entry.commit == *second.hash().as_bytes())
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.current && entry.commit == *restored.hash().as_bytes())
    );
    let mut saw_ten = false;
    for entry in entries {
        let HostReply::RevisionPreviewed(Ok(id)) = request(
            block,
            HostRequest::PreviewRevision {
                block_id: block.into_bytes(),
                commit: entry.commit,
            },
        ) else {
            panic!("retained history previews");
        };
        let id = Uuid::from_bytes(id);
        saw_ten |= CounterContent::decode(&content(id).unwrap().bytes)
            .unwrap()
            .root()
            .value()
            == 10;
        revisions::release(id);
    }
    assert!(saw_ten, "the unsaved edits were sealed before restoring");
    let other = Uuid::new_v4();
    open(other, CounterContent::CONTENT_TYPE);
    wait_for_count(other, 0);
    assert!(
        matches!(
            request(
                other,
                HostRequest::PreviewRevision {
                    block_id: other.into_bytes(),
                    commit: *first.hash().as_bytes()
                }
            ),
            HostReply::RevisionPreviewed(Err(_))
        ),
        "a commit from another block is rejected"
    );
    revisions::release(preview);
    revisions::collect_released();
    assert!(content(preview).is_none());
    assert!(!access(preview).can_view());
}
