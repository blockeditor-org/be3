use super::*;
use crate::create_binding;

#[test]
fn a_binding_inside_an_effect_tracks_for_itself() {
    let _root = RootScope::new();
    let outer = create_signal(0);
    let inner = create_signal(0);
    let outer_runs = Rc::new(Cell::new(0));
    let inner_runs = Rc::new(Cell::new(0));

    create_effect({
        let (outer_runs, inner_runs) = (outer_runs.clone(), inner_runs.clone());
        move || {
            outer.get();
            outer_runs.set(outer_runs.get() + 1);
            let inner_runs = inner_runs.clone();
            create_binding(move || {
                inner.get();
                inner_runs.set(inner_runs.get() + 1);
            });
        }
    });
    flush();
    inner.set(1);
    flush();

    assert_eq!(
        outer_runs.get(),
        1,
        "what the binding read does not wake the effect"
    );
    assert_eq!(inner_runs.get(), 2);

    outer.set(1);
    flush();
    inner.set(2);
    flush();
    assert_eq!(outer_runs.get(), 2);
    assert_eq!(
        inner_runs.get(),
        4,
        "the binding the effect's first run made went with that run"
    );
}
