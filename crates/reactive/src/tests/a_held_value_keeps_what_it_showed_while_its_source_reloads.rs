use super::*;
use crate::{Pending, Ready, hold};

#[test]
fn a_held_value_keeps_what_it_showed_while_its_source_reloads() {
    let _root = RootScope::new();
    let source = create_signal::<Ready<String>>(Ok("first".to_owned()));
    let held = hold(source);
    let runs = Rc::new(Cell::new(0));
    create_effect({
        let runs = runs.clone();
        move || {
            held.get();
            runs.set(runs.get() + 1);
        }
    });

    source.set(Err(Pending::Loading));
    assert_eq!(held.get(), "first");
    assert_eq!(runs.get(), 1);

    source.set(Ok("second".to_owned()));
    assert_eq!(held.get(), "second");
    assert_eq!(runs.get(), 2);
}
