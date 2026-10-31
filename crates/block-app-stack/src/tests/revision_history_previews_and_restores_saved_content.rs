use super::*;
use revisions::{Answered, Asked};

fn request(block: Uuid, request: Asked) -> Answered {
    let (send, receive) = std::sync::mpsc::channel();
    revisions::request(block, request, move |reply| {
        send.send(reply).unwrap();
    });
    receive
        .recv()
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
    let Answered::Listed(Ok(entries)) = request(block, Asked::List) else {
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

    let Answered::Previewed(Ok(preview)) = request(block, Asked::Preview(*first.hash().as_bytes()))
    else {
        panic!("the revision previews");
    };
    let old = content(preview).expect("preview content exists");
    assert_eq!(
        CounterContent::decode(old.bytes()).unwrap().root().value(),
        5
    );
    assert!(!access(preview).can_edit());
    add(block, 3);
    wait_for_count(block, 10);
    assert_eq!(
        content(preview).unwrap().bytes().to_vec(),
        old.bytes().to_vec(),
        "live edits do not reach the preview"
    );
    assert!(matches!(
        request(block, Asked::Restore(*first.hash().as_bytes())),
        Answered::Restored(Ok(()))
    ));
    wait_for_count(block, 5);
    let restored = node(block).unwrap().head.unwrap();
    assert_ne!(restored, first);
    let Answered::Listed(Ok(entries)) = request(block, Asked::List) else {
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
        let Answered::Previewed(Ok(id)) = request(block, Asked::Preview(entry.commit)) else {
            panic!("retained history previews");
        };
        saw_ten |= CounterContent::decode(content(id).unwrap().bytes())
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
            request(other, Asked::Preview(*first.hash().as_bytes())),
            Answered::Previewed(Err(_))
        ),
        "a commit from another block is rejected"
    );
    revisions::release(preview);
    revisions::collect_released(|_| {});
    assert!(content(preview).is_none());
    assert!(!access(preview).can_view());
}
