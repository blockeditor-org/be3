use super::*;
use crate::{Boundary, Pending, create_source};

#[test]
fn a_boundary_reports_a_failure_and_forgets_reads_whose_scope_went() {
    let _root = RootScope::new();
    let failing = create_source::<u32>();
    failing.set(Err(Pending::Failed("refused".to_owned())));
    let loading = create_source::<u32>();
    let scope = Scope::new();
    let (boundary, inner) = scope.run(|| {
        let boundary = Boundary::provide();
        let _ = loading.get_untracked();
        let inner = Scope::new();
        inner.run(|| {
            let _ = failing.get_untracked();
        });
        (boundary, inner)
    });

    assert_eq!(boundary.state(), Err(Pending::Failed("refused".to_owned())));

    inner.dispose();
    assert_eq!(boundary.state(), Err(Pending::Loading));

    loading.set(Ok(1));
    assert_eq!(boundary.state(), Ok(()));
}
