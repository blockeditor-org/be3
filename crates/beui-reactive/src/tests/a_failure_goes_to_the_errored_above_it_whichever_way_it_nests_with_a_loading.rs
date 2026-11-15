use super::*;
use crate::{Boundary, ErrorBoundary, Pending, Ready, create_source};

fn failed() -> Ready<()> {
    Err(Pending::Failed("refused".to_owned()))
}

#[test]
fn a_failure_goes_to_the_errored_above_it_whichever_way_it_nests_with_a_loading() {
    let _root = RootScope::new();
    for revealed in [false, true] {
        for errored_outside in [true, false] {
            let source = create_source::<u32>();
            let scope = Scope::new();
            let (errored, loading) = scope.run(|| {
                let (errored, loading) = match errored_outside {
                    true => (ErrorBoundary::provide(), Boundary::provide()),
                    false => {
                        let loading = Boundary::provide();
                        (ErrorBoundary::provide(), loading)
                    }
                };
                if revealed {
                    loading.reveal();
                }
                create_effect(move || {
                    let _ = source.get();
                });
                (errored, loading)
            });
            flush();
            source.set(Err(Pending::Failed("refused".to_owned())));
            flush();
            assert_eq!(
                errored.failure(),
                Some("refused".to_owned()),
                "errored outside: {errored_outside}, revealed: {revealed}"
            );
            assert_ne!(loading.state(), failed(), "the Loading does not show it");
            scope.dispose();
        }
    }
}
