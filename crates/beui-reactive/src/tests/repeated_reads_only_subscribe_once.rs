use super::*;

#[test]
fn repeated_reads_only_subscribe_once() {
    let data = Scope::new();
    let value = data.run(|| create_signal(1));
    let scope = Scope::new();
    scope.run(|| {
        create_effect(move || {
            value.get();
            value.get();
        });
    });
    for next in 2..10 {
        value.set(next);
        assert_eq!(value.subscribers(), 1);
    }
    scope.dispose();
    assert_eq!(value.subscribers(), 0);
}
