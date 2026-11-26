use super::*;
use crate::{Boundary, Pending, create_source};

#[test]
fn a_diamond_over_a_pending_source_is_pending_until_it_settles_and_wakes_once() {
    let _root = RootScope::new();
    let source = create_source::<u32>();
    let runs = Rc::new(Cell::new(0));
    let seen = runs.clone();
    let scope = Scope::new();
    let (loading, sum) = scope.run(|| {
        let loading = Boundary::provide();
        let left = create_memo(move || source.get().map(|value| value + 1));
        let right = create_memo(move || source.get().map(|value| value * 2));
        let sum = create_memo(move || Ok::<_, Pending>(left.get()? + right.get()?));
        create_effect(move || {
            seen.set(seen.get() + 1);
            let _ = sum.get();
        });
        (loading, sum)
    });
    flush();
    assert_eq!(loading.state(), Err(Pending::Loading));
    assert_eq!(runs.get(), 1);

    source.set(Ok(3));
    flush();
    assert_eq!(loading.state(), Ok(()));
    assert_eq!(sum.get_untracked(), Ok(10));
    assert_eq!(runs.get(), 2, "the reader wakes once, not once per side");
}
