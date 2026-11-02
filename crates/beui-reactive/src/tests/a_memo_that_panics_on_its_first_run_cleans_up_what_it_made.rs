use super::*;

#[test]
fn a_memo_that_panics_on_its_first_run_cleans_up_what_it_made() {
    let _root = RootScope::new();
    let cleaned = Rc::new(Cell::new(false));

    let result = catch_unwind(AssertUnwindSafe({
        let cleaned = cleaned.clone();
        move || {
            create_memo(move || {
                let cleaned = cleaned.clone();
                on_cleanup(move || cleaned.set(true));
                panic!("the first run fails");
            })
        }
    }));

    assert!(result.is_err());
    assert!(
        cleaned.get(),
        "what the failed run made is cleaned up at once"
    );
}
