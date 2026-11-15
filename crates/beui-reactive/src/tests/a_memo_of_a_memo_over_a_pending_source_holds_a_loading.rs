use super::*;
use crate::{Boundary, Pending, create_source};

#[test]
fn a_memo_of_a_memo_over_a_pending_source_holds_a_loading() {
    let _root = RootScope::new();
    let source = create_source::<u32>();
    let scope = Scope::new();
    let loading = scope.run(|| {
        let loading = Boundary::provide();
        let inner = create_memo(move || source.get().map(|value| value + 1));
        let outer = create_memo(move || inner.get().map(|value| value * 2));
        create_effect(move || {
            let _ = outer.get();
        });
        loading
    });
    flush();
    assert_eq!(loading.state(), Err(Pending::Loading));
    source.set(Ok(1));
    flush();
    assert_eq!(loading.state(), Ok(()));
}
