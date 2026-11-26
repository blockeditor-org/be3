use super::*;
use crate::{Boundary, Pending, create_binding, create_source};

#[test]
fn a_memo_over_a_pending_value_is_pending_until_it_settles() {
    let _root = RootScope::new();
    let rows = create_source::<Vec<u32>>();
    let scope = Scope::new();
    let boundary = scope.run(|| {
        let boundary = Boundary::provide();
        let count = create_memo(move || Ok::<_, Pending>(rows.get()?.len()));
        // Handling the pending value still waits: the memo read a source that is pending.
        let shown = create_memo(move || rows.with(|rows| rows.as_ref().map_or(0, Vec::len)));
        create_binding(move || {
            count.track();
            shown.track();
        });
        boundary
    });
    assert_eq!(boundary.state(), Err(Pending::Loading));
    rows.set(Ok(vec![1, 2]));
    assert_eq!(boundary.state(), Ok(()));
}
