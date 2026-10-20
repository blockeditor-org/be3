use super::*;

#[test]
fn non_clone_values_can_be_read_and_updated() {
    let _root = RootScope::new();
    struct Value(usize);
    let read = create_signal(Value(1));
    assert_eq!(
        read.update(|value| {
            value.0 += 1;
            value.0
        }),
        2
    );
    assert_eq!(read.with(|value| value.0), 2);
    read.set_unconditionally(Value(3));
    assert_eq!(read.with_untracked(|value| value.0), 3);
}
