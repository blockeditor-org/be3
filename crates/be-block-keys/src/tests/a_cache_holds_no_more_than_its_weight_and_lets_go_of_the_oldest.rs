use crate::lru::Lru;

/// A cache bounded by weight holds no more than its capacity in total, lets go of what was used
/// longest ago first, keeps what is used again, and keeps nothing heavier than it holds.
#[test]
fn a_cache_holds_no_more_than_its_weight_and_lets_go_of_the_oldest() {
    let mut cache = Lru::new(10);
    cache.insert("a", 1, 4);
    cache.insert("b", 2, 4);
    assert_eq!(cache.get(&"a"), Some(&1), "a is used again");
    cache.insert("c", 3, 4);
    assert_eq!(cache.get(&"b"), None, "b, used longest ago, is let go");
    assert_eq!(cache.get(&"a"), Some(&1));
    assert_eq!(cache.get(&"c"), Some(&3));
    assert_eq!(cache.weight(), 8);

    cache.insert("a", 4, 2);
    assert_eq!(cache.weight(), 6, "putting a key again replaces its weight");
    cache.insert("heavy", 5, 11);
    assert_eq!(
        cache.get(&"heavy"),
        None,
        "nothing heavier than the cache is kept"
    );
    assert_eq!(cache.len(), 2);
    cache.insert("whole", 6, 10);
    assert_eq!(cache.len(), 1, "one that fills it lets go of the rest");
    assert_eq!(cache.weight(), 10);
    cache.remove(&"whole");
    assert!(cache.is_empty());
    assert_eq!(cache.weight(), 0);
}
