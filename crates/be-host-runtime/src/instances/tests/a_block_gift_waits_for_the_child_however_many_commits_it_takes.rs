use super::*;

#[test]
fn a_block_gift_waits_for_the_child_however_many_commits_it_takes() {
    let mut instances = placed();
    instances.next_screens(PASS);
    tell(
        &mut instances,
        ShellMessage::FocusBlockChild(Uuid::now_v7().into_bytes()),
    );
    assert!(
        instances.gift_pending(INSTANCE),
        "the editor asked for the keyboard for a child it has yet to place"
    );
    for _ in 0..gift::GIFT_COMMITS * 4 {
        instances.settle_gifts(&[INSTANCE]);
    }
    assert_eq!(
        instances.expire_gifts(host::now() + gift::GIFT_WAIT * 10),
        None
    );
    assert!(
        instances.gift_pending(INSTANCE),
        "a busy host can take any number of commits and any time to make the block and place \
         its child, and the keys typed meanwhile wait for it"
    );
    instances.end_gift(INSTANCE, gift::Keys::Return);
    assert!(
        !instances.gift_pending(INSTANCE),
        "the person moving on ends it"
    );
}
