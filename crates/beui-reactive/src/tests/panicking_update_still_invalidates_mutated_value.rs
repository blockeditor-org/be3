use super::*;

#[test]
fn panicking_update_still_invalidates_mutated_value() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let value = create_signal(1);
    let memo = scope.run(|| create_memo(move || value.get() * 2));
    assert!(
        catch_unwind(AssertUnwindSafe(|| value.update(|value| {
            *value = 3;
            panic!("update failed");
        })))
        .is_err()
    );
    assert_eq!(memo.get(), 6);
}
