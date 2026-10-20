use super::*;

#[test]
fn memo_writes_are_rejected_even_when_untracked() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let write = create_signal(0);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || scope.run(|| create_memo(move || untrack(|| write.set(1))))
        ))
        .is_err()
    );
    let read = create_signal(0);
    read.set(2);
    assert_eq!(read.get(), 2);
}
