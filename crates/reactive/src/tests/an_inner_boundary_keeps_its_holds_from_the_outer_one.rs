use super::*;
use crate::{Boundary, Pending, Ready, hold};

#[test]
fn an_inner_boundary_keeps_its_holds_from_the_outer_one() {
    let _root = RootScope::new();
    let source = create_signal::<Ready<u32>>(Err(Pending::Loading));
    let scope = Scope::new();
    let (outer, inner, _nested) = scope.run(|| {
        let outer = Boundary::provide();
        let nested = Scope::new();
        let inner = nested.run(|| {
            let inner = Boundary::provide();
            hold(source);
            inner
        });
        (outer, inner, nested)
    });

    assert_eq!(outer.state(), Ok(()));
    assert_eq!(inner.state(), Err(Pending::Loading));
}
