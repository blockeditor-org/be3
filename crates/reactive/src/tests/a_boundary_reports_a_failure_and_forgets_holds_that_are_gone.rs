use super::*;
use crate::{Boundary, Pending, Ready, hold};

#[test]
fn a_boundary_reports_a_failure_and_forgets_holds_that_are_gone() {
    let _root = RootScope::new();
    let failing = create_signal::<Ready<u32>>(Err(Pending::Failed("refused".to_owned())));
    let loading = create_signal::<Ready<u32>>(Err(Pending::Loading));
    let scope = Scope::new();
    let (boundary, inner) = scope.run(|| {
        let boundary = Boundary::provide();
        hold(loading);
        let inner = Scope::new();
        inner.run(|| hold(failing));
        (boundary, inner)
    });

    assert_eq!(boundary.state(), Err(Pending::Failed("refused".to_owned())));

    inner.dispose();
    assert_eq!(boundary.state(), Err(Pending::Loading));

    loading.set(Ok(1));
    assert_eq!(boundary.state(), Ok(()));
}
