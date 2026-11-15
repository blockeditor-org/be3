use super::*;
use crate::{Boundary, Pending, create_source};

#[test]
fn an_inner_boundary_keeps_its_reads_from_the_outer_one() {
    let _root = RootScope::new();
    let source = create_source::<u32>();
    let scope = Scope::new();
    let (outer, inner, _nested) = scope.run(|| {
        let outer = Boundary::provide();
        let nested = Scope::new();
        let inner = nested.run(|| {
            let inner = Boundary::provide();
            let _ = source.get_untracked();
            inner
        });
        (outer, inner, nested)
    });

    assert_eq!(outer.state(), Ok(()));
    assert_eq!(inner.state(), Err(Pending::Loading));
}
