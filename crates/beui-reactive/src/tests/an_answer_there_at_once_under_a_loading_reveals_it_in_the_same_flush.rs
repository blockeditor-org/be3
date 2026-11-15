use super::executor::install;
use super::*;
use crate::{Boundary, create_async_memo, create_binding};

#[test]
fn an_answer_there_at_once_under_a_loading_reveals_it_in_the_same_flush() {
    let _root = RootScope::new();
    install();
    let scope = Scope::new();
    let loading = scope.run(|| {
        let loading = Boundary::provide();
        let memo = create_async_memo(move || async move { Ok(7_u32) });
        create_binding(move || {
            let _ = memo.get();
        });
        loading
    });
    assert_eq!(
        loading.state(),
        Ok(()),
        "nothing it read is pending, with no flush between"
    );
}
