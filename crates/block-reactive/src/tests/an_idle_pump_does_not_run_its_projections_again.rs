use super::*;

#[test]
fn an_idle_pump_does_not_run_its_projections_again() {
    let client = client();
    let block = client.create_block(Calendar::default());
    let source = BlockSource::new(block.clone(), || {});
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let events = source.project(move |calendar: &Calendar| {
        counted.set(counted.get() + 1);
        calendar.events().len()
    });

    assert_eq!(runs.get(), 1);
    for _ in 0..3 {
        source.pump();
    }
    assert_eq!(runs.get(), 1);
    assert_eq!(events.get_untracked(), 0);

    block.operate(add_event("write"));
    block.operate(add_event("review"));
    source.pump();
    assert_eq!(runs.get(), 2);
    assert_eq!(events.get_untracked(), 2);

    source.pump();
    assert_eq!(runs.get(), 2);
}
