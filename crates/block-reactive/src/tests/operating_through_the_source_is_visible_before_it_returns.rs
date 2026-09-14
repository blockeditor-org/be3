use super::*;

#[test]
fn operating_through_the_source_is_visible_before_it_returns() {
    let client = client();
    let source = BlockSource::new(client.create_block(Counter::default()), || {});
    let count = source.project(Counter::count);

    source.operate(CounterOperation::Increment);
    assert_eq!(count.get_untracked(), 1);
    source.operate(CounterOperation::Decrement);
    assert_eq!(count.get_untracked(), 0);
}
