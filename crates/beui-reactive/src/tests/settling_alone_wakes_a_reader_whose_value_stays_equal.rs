use super::*;
use crate::{Boundary, Pending, create_source};

#[test]
fn settling_alone_wakes_a_reader_whose_value_stays_equal() {
    let _root = RootScope::new();
    let source = create_source::<u32>();
    let runs = Rc::new(Cell::new(0));
    let seen = runs.clone();
    let scope = Scope::new();
    let loading = scope.run(|| {
        let loading = Boundary::provide();
        // Handles the pending source, so its value is the same before and after: it is still
        // pending until the source settles.
        let shown = create_memo(move || source.get().unwrap_or(0).min(0));
        create_effect(move || {
            seen.set(seen.get() + 1);
            let _ = shown.get();
        });
        loading
    });
    flush();
    assert_eq!(loading.state(), Err(Pending::Loading));
    assert_eq!(runs.get(), 1);

    source.set(Ok(5));
    flush();
    assert_eq!(
        runs.get(),
        2,
        "only whether it is there changed, and that wakes the reader"
    );
    assert_eq!(loading.state(), Ok(()));
}
