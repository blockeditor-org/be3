use super::*;

#[test]
fn repeated_reads_only_subscribe_once() {
    let data = Scope::new();
    let (value, set_value) = data.run(|| create_signal(1));
    let scope = Scope::new();
    scope.run(|| {
        create_effect(move || {
            value.get();
            value.get();
        });
    });
    for next in 2..10 {
        set_value.set(next);
        assert_eq!(value.source().subscribers.borrow().len(), 1);
    }
    scope.dispose();
    assert!(value.source().subscribers.borrow().is_empty());
}
