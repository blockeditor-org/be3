use super::*;

#[test]
fn effects_can_write_signals_and_settle_without_recursion() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let value = create_signal(0);
    scope.run(|| {
        create_effect(move || {
            let current = value.get();
            if current < 5 {
                value.set(current + 1);
            }
        });
    });
    assert_eq!(value.get(), 5);
}
