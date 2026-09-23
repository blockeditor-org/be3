use super::*;

#[test]
fn two_counters_merge_by_keeping_both_sides_of_the_count() {
    let base = edited(&CounterContent::default(), [Counter::add(10)]);
    let ours = edited(&base, [Counter::add(1), Counter::add(1)]);
    let theirs = edited(&base, [Counter::add(-4)]);

    let MergeResult::Clean(merged) = CounterContent::merge3(&base, &ours, &theirs) else {
        panic!("a counter never conflicts");
    };

    assert_eq!(merged.root().value(), 8);
}
