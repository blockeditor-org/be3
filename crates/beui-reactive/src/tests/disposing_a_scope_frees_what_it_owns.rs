use super::*;

#[test]
fn disposing_a_scope_frees_what_it_owns() {
    let before = crate::arena::live();
    let scope = Scope::new();
    let count = scope.run(|| create_signal(1));
    let (doubled, selected) = scope.run(|| {
        (
            create_memo(move || count.get() * 2),
            create_selector(move || count.get()),
        )
    });
    count.set(2);
    assert_eq!(doubled.get(), 4);
    assert!(selected.is_selected(&2));
    assert_eq!(crate::arena::live(), before + 4);
    scope.dispose();
    assert_eq!(crate::arena::live(), before, "the scope's values are freed");
    assert!(!count.is_alive() && !doubled.is_alive());
    let read = catch_unwind(AssertUnwindSafe(|| count.get()));
    assert!(read.is_err(), "a freed signal cannot be read");
    count.set(3);
}
