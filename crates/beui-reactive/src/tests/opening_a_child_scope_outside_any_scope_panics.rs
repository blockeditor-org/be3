use super::*;

#[test]
fn opening_a_child_scope_outside_any_scope_panics() {
    let opened = catch_unwind(AssertUnwindSafe(|| crate::child_scope(|| ())));
    assert!(
        opened.is_err(),
        "a child scope with no parent would never be disposed"
    );
}
