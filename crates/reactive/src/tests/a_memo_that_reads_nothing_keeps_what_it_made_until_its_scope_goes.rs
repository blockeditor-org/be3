use super::*;

#[test]
fn a_memo_that_reads_nothing_keeps_what_it_made_until_its_scope_goes() {
    let _root = RootScope::new();
    let cleaned = Rc::new(Cell::new(false));
    let scope = Scope::new();

    let memo = scope.run({
        let cleaned = cleaned.clone();
        move || {
            create_memo(move || {
                let cleaned = cleaned.clone();
                on_cleanup(move || cleaned.set(true));
                7
            })
        }
    });

    assert_eq!(memo.get_untracked(), 7);
    assert!(!cleaned.get());
    drop(scope);
    assert!(
        cleaned.get(),
        "what the memo made goes with the scope that owns it"
    );
}
