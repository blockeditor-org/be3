use super::*;
use be_block_content::BlockContent;
use be_type_example_counter::CounterContent;

/// An editor holds two replicas of a block in two content types, the earlier of one the host
/// does not link: once the later ends, its link goes rather than send the earlier a snapshot of
/// the later's type.
#[test]
fn an_earlier_replica_of_a_type_the_host_does_not_link_hears_nothing_of_the_later_ones() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let own = Uuid::new_v4();
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);
    next_screens(&mut instances);
    let (block, _) = saved_counter();
    let unlinked = Uuid::new_v4();
    let earlier = hold(&mut instances, block, unlinked);
    let linked = |instances: &Instances| {
        instances
            .entries
            .get(&INSTANCE)
            .and_then(|entry| entry.watched.get(&block))
            .map(|link| link.content_type)
    };
    assert_eq!(linked(&instances), None, "the host links no {unlinked}");

    let later = hold(&mut instances, block, CounterContent::CONTENT_TYPE);
    assert_eq!(linked(&instances), Some(CounterContent::CONTENT_TYPE));
    instances.ended(later);
    assert_eq!(
        instances
            .entries
            .get(&INSTANCE)
            .and_then(|entry| entry.held.replica(block)),
        Some(earlier)
    );
    assert_eq!(linked(&instances), None, "the later replica's link went");
    assert!(from_replica(&mut instances, block).is_empty());
}
