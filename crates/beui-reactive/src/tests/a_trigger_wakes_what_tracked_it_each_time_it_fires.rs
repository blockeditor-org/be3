use super::*;
use crate::create_trigger;

#[test]
fn a_trigger_wakes_what_tracked_it_each_time_it_fires() {
    let _root = RootScope::new();
    let trigger = create_trigger();
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    create_effect(move || {
        trigger.track();
        counted.set(counted.get() + 1);
    });
    flush();
    assert_eq!(runs.get(), 1);
    trigger.notify();
    trigger.notify();
    flush();
    assert_eq!(
        runs.get(),
        2,
        "two notifications before a flush run it once"
    );
    trigger.notify();
    flush();
    assert_eq!(runs.get(), 3);
}
