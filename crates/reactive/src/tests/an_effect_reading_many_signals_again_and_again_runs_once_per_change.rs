use super::*;

#[test]
fn an_effect_reading_many_signals_again_and_again_runs_once_per_change() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let signals: Rc<Vec<_>> = Rc::new((0..100).map(create_signal).collect());
    let shared = create_signal(0);
    let runs = Rc::new(Cell::new(0));
    let inner_runs = Rc::new(Cell::new(0));
    scope.run(|| {
        let (signals, runs, inner_runs) = (signals.clone(), runs.clone(), inner_runs.clone());
        create_effect(move || {
            runs.set(runs.get() + 1);
            for _ in 0..3 {
                for signal in signals.iter() {
                    signal.get();
                    shared.get();
                }
            }
            let inner_runs = inner_runs.clone();
            create_effect(move || {
                inner_runs.set(inner_runs.get() + 1);
                shared.get();
            });
            shared.get();
        });
    });
    assert_eq!(runs.get(), 1);
    signals[57].set(1000);
    flush();
    assert_eq!(
        runs.get(),
        2,
        "a change to one signal read many times runs the effect once"
    );
    shared.set(1);
    flush();
    assert_eq!(
        runs.get(),
        3,
        "a signal also read by an inner effect runs the outer one once"
    );
}
