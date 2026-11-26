use super::*;

#[test]
fn a_watch_is_not_answered_again_what_it_was_answered() {
    let mut store = Recorded::default();
    let block = Uuid::from_u128(3);
    let subject = Subject::History(block);
    call(
        &mut store,
        INSTANCE,
        Call::BlockWatchNew {
            resource: wire::BlockWatch(5),
            subject: BlockSubject::History(block.into_bytes()),
        },
    );
    assert_eq!(store.heard, ["watching 5"]);
    assert_eq!(store.blocks.watches().collect::<Vec<_>>(), [(5, subject)]);
    assert!(answers(&mut store, &[5]).is_empty(), "nothing offered yet");

    store.blocks.offer(subject, &history(false));
    store.blocks.offer(subject, &history(true));
    let answered = answers(&mut store, &[5]);
    assert_eq!(answered.len(), 1);
    let BlockValue::History(latest) = decode(&answered[0].1).expect("an answer decodes") else {
        panic!("a history watch is answered a history");
    };
    assert!(latest.can_undo, "only the latest value is answered");

    store.blocks.offer(subject, &history(true));
    assert!(answers(&mut store, &[5]).is_empty(), "the same value again");
    store.blocks.offer(subject, &history(false));
    assert_eq!(answers(&mut store, &[5]).len(), 1, "a new value");

    assert!(ended(&mut store, 5));
    store.blocks.offer(subject, &history(true));
    assert!(answers(&mut store, &[5]).is_empty(), "a watch gone");
}
