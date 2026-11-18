use super::*;
use crate::{Trackable, create_trigger};

#[test]
fn a_trackable_follows_a_trigger_or_a_value_alike() {
    let _root = RootScope::new();
    let trigger = create_trigger();
    let number = create_signal(0);
    let doubled = create_memo(move || number.get() * 2);
    let runs = Rc::new(Cell::new(0));
    let tracks: Vec<Trackable> = vec![trigger.into(), number.into(), doubled.into()];
    {
        let runs = runs.clone();
        create_effect(move || {
            for track in &tracks {
                track.track();
            }
            runs.set(runs.get() + 1);
        });
    }
    flush();
    assert_eq!(runs.get(), 1, "an effect runs once to start");

    trigger.notify();
    flush();
    assert_eq!(runs.get(), 2, "every notify of a trigger wakes it");

    number.set(1);
    flush();
    assert_eq!(
        runs.get(),
        3,
        "a signal and the memo over it wake it once together"
    );
    number.set(1);
    flush();
    assert_eq!(runs.get(), 3, "a signal set to what it holds wakes nothing");
}
