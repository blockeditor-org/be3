use super::*;

#[test]
fn a_queued_block_the_mirror_cannot_place_is_dropped_and_counted() {
    let harness = Harness::start();
    harness.connect();
    // Queued as when another device's index asks for its text, but no block has this id: the
    // mirror is asked for it, lists nothing, and the block leaves the queue, counted.
    let nowhere = Uuid::new_v4();
    with_shared_mut(|shared| shared.search.reread([nowhere]));
    retry_now();
    crate::wait_until(|shared| (shared.search.unplaceable == 1).then_some(()));
    crate::wait_until(|shared| shared.search.indexed().then_some(()));
}
