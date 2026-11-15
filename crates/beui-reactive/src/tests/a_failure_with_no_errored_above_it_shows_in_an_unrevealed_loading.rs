use super::*;
use crate::{Boundary, Pending, create_source};

#[test]
fn a_failure_with_no_errored_above_it_shows_in_an_unrevealed_loading() {
    let _root = RootScope::new();
    let source = create_source::<u32>();
    let scope = Scope::new();
    let loading = scope.run(|| {
        let loading = Boundary::provide();
        create_effect(move || {
            let _ = source.get();
        });
        loading
    });
    flush();
    assert_eq!(loading.state(), Err(Pending::Loading));
    source.set(Err(Pending::Failed("refused".to_owned())));
    flush();
    assert_eq!(loading.state(), Err(Pending::Failed("refused".to_owned())));
}
