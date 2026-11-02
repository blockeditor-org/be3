use super::*;

#[test]
fn an_edit_made_across_a_takeover_is_kept() {
    let harness = Harness::start();
    let block = Uuid::new_v4();
    let mut owner = Outside::counter(&harness, block);
    assert!(owner.is_owner());
    let outside = owner.peer();

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    add(block, 10);
    owner.until("sequenced this peer's add", |seen| seen.count == Some(10));
    wait_until("had its add sequenced", |shared| {
        counted(shared, block) == Some(10) && shared.unsealed == 0
    });

    owner.seal();
    owner.leave();
    add(block, 100);

    wait_until("saved what it typed across the takeover", |shared| {
        counted(shared, block) == Some(110) && shared.unsealed == 0
    });
    let stored = owner.block_on(async {
        outside
            .open::<CounterContent>(block)
            .await
            .expect("the counter reads back")
            .expect("the counter has content")
            .root()
            .value()
    });
    assert_eq!(stored, 110);
}
