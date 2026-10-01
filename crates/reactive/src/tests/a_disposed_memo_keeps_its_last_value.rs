use super::*;

#[test]
fn a_disposed_memo_keeps_its_last_value() {
    let scope = Scope::new();
    let (count, set_count) = create_signal(1);
    let memo = scope.run(|| create_memo(move || count.get() * 10));
    scope.dispose();
    set_count.set(2);
    assert_eq!(
        memo.get(),
        10,
        "a disposed memo answers the value it last computed"
    );
}
