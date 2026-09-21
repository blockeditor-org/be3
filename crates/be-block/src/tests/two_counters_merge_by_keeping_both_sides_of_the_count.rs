use super::*;

#[test]
fn two_counters_merge_by_keeping_both_sides_of_the_count() {
    let base = CounterContent::new(10);
    let ours = counted(base, &[CounterOp::Add { by: 1 }, CounterOp::Add { by: 1 }]);
    let theirs = counted(base, &[CounterOp::Add { by: -4 }]);

    let MergeResult::Clean(merged) = CounterContent::merge3(&base, &ours, &theirs) else {
        panic!("a counter never conflicts");
    };

    assert_eq!(merged.count(), 8);
}
