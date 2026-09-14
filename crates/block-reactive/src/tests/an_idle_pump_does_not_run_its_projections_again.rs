use super::*;

#[test]
fn an_idle_pump_does_not_run_its_projections_again() {
    let client = client();
    let block = client.create_block(Counter::default());
    let source = BlockSource::new(block.clone(), || {});
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let count = source.project(move |counter| {
        counted.set(counted.get() + 1);
        counter.count()
    });

    assert_eq!(runs.get(), 1);
    for _ in 0..3 {
        source.pump();
    }
    assert_eq!(runs.get(), 1);
    assert_eq!(count.get_untracked(), 0);

    block.operate(CounterOperation::Increment);
    block.operate(CounterOperation::Increment);
    source.pump();
    assert_eq!(runs.get(), 2);
    assert_eq!(count.get_untracked(), 2);

    source.pump();
    assert_eq!(runs.get(), 2);
}
